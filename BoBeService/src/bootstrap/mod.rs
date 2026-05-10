//! Application bootstrap — wires all dependencies and starts background services.
//!
//! Submodules by lifecycle phase:
//! - `database` — pool creation and migrations
//! - `infra`    — SSE, mDNS, config arc-swap
//! - `repos`    — repository trait object construction
//! - `wiring`   — services, learners, triggers, runtime session assembly

mod database;
mod infra;
mod repos;
mod wiring;

use std::sync::Arc;

use tracing::{info, warn};

use crate::app_state::AppState;
use crate::config::Config;
use crate::error::AppError;

pub(crate) async fn run(config: Config) -> Result<Arc<AppState>, AppError> {
    let pool = database::connect_and_apply_schema(&config.database.url).await?;

    let infra = infra::Infrastructure::build(&config)?;
    let repos = repos::Repositories::from_pool(&pool);

    if config.mcp.enabled
        && let Err(e) =
            crate::mcp::config::ensure_mcp_config_exists(config.mcp.config_file.as_deref())
    {
        warn!(error = %e, "bootstrap.ensure_mcp_config_failed");
    }

    // Memory.md + WorkerRegistry constructed up-front so `wiring` can
    // pass `workers` into the components that need it.
    let memory_file = {
        let path = crate::util::paths::bobe_data_dir().join("memory.md");
        crate::copilot::memory_file::MemoryFile::new(path)
    };
    // Default SKILL.md files for worker classes that ship with one.
    // Idempotent: existing skills are never overwritten.
    crate::copilot::skills::ensure_skills(&crate::util::paths::bobe_data_dir()).await;
    // Parse mcp.json once at boot. The Copilot SDK takes the resulting
    // map via `SessionConfig::mcp_servers` and owns process spawn +
    // tool dispatch — BoBe no longer manages MCP server lifecycles.
    let mcp_servers = load_mcp_servers_for_sdk(&config);
    let workers = {
        let data_dir = crate::util::paths::bobe_data_dir();
        crate::copilot::registry::WorkerRegistry::new(
            Arc::clone(&infra.config_arc),
            Arc::clone(&memory_file),
            data_dir,
            mcp_servers,
        )
    };

    // Best-effort cleanup of chat session-id files older than the
    // retention window. Removes both the SDK-side session state and
    // the local pointer; logs and continues on failure.
    workers.prune_old_chat_sessions().await;

    let wired = wiring::wire(&config, &infra, &repos, Arc::clone(&workers)).await;

    // Wire ConfigManager → WorkerRegistry hot-swap. Engine-config changes
    // (cloud↔local, per-class models, base URL, offline flag) trigger a
    // registry reload in the background — the CLI restarts and sessions
    // re-spawn against the new config on next access. No daemon restart.
    {
        let registry_for_listener = Arc::clone(&workers);
        wired.config_manager.set_engine_change_listener(move || {
            let registry = Arc::clone(&registry_for_listener);
            tokio::spawn(async move {
                registry.reload().await;
            });
        });
    }

    if config.seed_default_documents {
        if let Err(e) = crate::db::seeding::seed_default_souls(repos.soul_repo.as_ref()).await {
            tracing::warn!(error = %e, "bootstrap.soul_seeding_failed");
        }
        if let Err(e) =
            crate::db::seeding::seed_default_user_profiles(repos.user_profile_repo.as_ref()).await
        {
            tracing::warn!(error = %e, "bootstrap.profile_seeding_failed");
        }
    }

    wired.wire_sse_callbacks(&infra.connection_manager).await;

    infra.mdns_announcer.start().await;

    print_banner(&infra.config_arc.load());

    // Ollama install service — orchestrates the wizard's local-mode
    // first-launch flow. Idle until POST /local-runtime/install fires.
    // Reuses a single reqwest::Client (rustls-tls) for both the binary
    // download and the Ollama API; cheap to share since reqwest::Client
    // is internally reference-counted.
    let ollama_install = {
        let data_dir = crate::util::paths::bobe_data_dir();
        let http = Arc::new(
            reqwest::Client::builder()
                .build()
                .map_err(|e| AppError::Internal(format!("reqwest client build: {e}")))?,
        );
        let binary = Arc::new(crate::binary_manager::BinaryManager::new(
            &data_dir,
            Arc::clone(&http),
        ));
        // Use the configured base URL when present, falling back to the
        // standard Ollama localhost endpoint. The provider URL has a `/v1`
        // suffix for OpenAI-compat; OllamaManager strips it via
        // `root_from_provider_url` to get the native API root.
        let base_url = config
            .engine
            .provider_base_url
            .as_deref()
            .map(crate::ollama_manager::OllamaManager::root_from_provider_url)
            .unwrap_or_else(|| "http://127.0.0.1:11434".to_string());
        let manager = Arc::new(crate::ollama_manager::OllamaManager::new(
            Arc::clone(&http),
            &base_url,
        ));
        crate::services::ollama_install_service::OllamaInstallService::new(binary, manager)
    };

    let state = Arc::new(AppState {
        db: pool,
        config: Arc::clone(&infra.config_arc),
        event_queue: infra.event_queue,
        connection_manager: infra.connection_manager,
        soul_repo: repos.soul_repo,
        user_profile_repo: repos.user_profile_repo,
        goals_service: wired.goals_service,
        runtime_session: wired.runtime_session,
        screen_capture: wired.screen_capture,
        config_manager: wired.config_manager,
        mcp_config_lock: Arc::new(tokio::sync::Mutex::new(())),
        mdns_announcer: infra.mdns_announcer,
        workers,
        memory_file,
        ollama_install,
    });

    Ok(state)
}

fn load_mcp_servers_for_sdk(
    config: &Config,
) -> std::collections::HashMap<String, github_copilot_sdk::types::McpServerConfig> {
    if !config.mcp.enabled {
        return std::collections::HashMap::new();
    }

    let path =
        match crate::mcp::config::resolve_mcp_config_path(config.mcp.config_file.as_deref())
        {
            Ok(p) => p,
            Err(e) => {
                warn!(error = %e, "bootstrap.mcp_config_path_resolution_failed");
                return std::collections::HashMap::new();
            }
        };

    let parsed = match crate::mcp::config::load_mcp_config(
        &path,
        &config.mcp.blocked_commands,
        &config.mcp.dangerous_env_keys,
    ) {
        Ok(servers) => servers,
        Err(e) => {
            warn!(error = %e, path = %path.display(), "bootstrap.mcp_config_parse_failed");
            return std::collections::HashMap::new();
        }
    };

    let count = parsed.len();
    let map = crate::mcp::config::to_sdk_mcp_servers(parsed);
    info!(count, "bootstrap.mcp_servers_registered_via_sdk");
    map
}

fn print_banner(config: &Config) {
    info!("═══════════════════════════════════════════════════════");
    info!("  BoBe Server Started");
    info!("  Engine: Copilot CLI (via github-copilot-sdk)");
    info!("  Capture enabled: {}", config.capture.enabled);
    info!("  Tools (MCP) enabled: {}", config.mcp.enabled);
    info!("═══════════════════════════════════════════════════════");
}
