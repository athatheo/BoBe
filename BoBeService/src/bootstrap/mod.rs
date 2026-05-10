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

    let memory_file = {
        let path = crate::util::paths::bobe_data_dir().join("memory.md");
        crate::copilot::memory_file::MemoryFile::new(path)
    };
    // Idempotent: existing skills are never overwritten.
    crate::copilot::skills::ensure_skills(&crate::util::paths::bobe_data_dir()).await;
    // SDK owns MCP process spawn + tool dispatch via `SessionConfig::mcp_servers`.
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

    workers.prune_old_chat_sessions().await;

    let wired = wiring::wire(&config, &infra, &repos, Arc::clone(&workers)).await;

    // Engine-config changes trigger registry reload; no daemon restart needed.
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
        // Strip `/v1` OpenAI-compat suffix to get the native Ollama API root.
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
