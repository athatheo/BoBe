//! Application bootstrap — wires all dependencies and starts background services.
//!
//! Split into focused submodules by lifecycle phase:
//! - `database`  — pool creation and migrations
//! - `infra`     — HTTP client, SSE, mDNS
//! - `repos`     — repository trait object construction
//! - `wiring`    — services, learners, triggers, runtime session assembly
//! - `integrity` — startup data-integrity checks (orphan cleanup)

mod database;
mod infra;
mod integrity;
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
            crate::tools::mcp::config::ensure_mcp_config_exists(config.mcp.config_file.as_deref())
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
    let workers = {
        let data_dir = crate::util::paths::bobe_data_dir();
        crate::copilot::registry::WorkerRegistry::new(Arc::clone(&memory_file), data_dir)
    };

    let wired = wiring::wire(&config, &infra, &repos, Arc::clone(&workers)).await;

    integrity::run(&pool, repos.agent_job_repo.as_ref()).await;

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

    wired.start_services(&config).await;
    wired.wire_sse_callbacks(&infra.connection_manager).await;

    infra.mdns_announcer.start().await;

    print_banner(&infra.config_arc.load());

    let state = Arc::new(AppState {
        db: pool,
        config: Arc::clone(&infra.config_arc),
        http_client: infra.http_client,
        event_queue: infra.event_queue,
        connection_manager: infra.connection_manager,
        conversation_repo: repos.conversation_repo,
        observation_repo: repos.observation_repo,
        cooldown_repo: repos.cooldown_repo,
        learning_state_repo: repos.learning_state_repo,
        agent_job_repo: repos.agent_job_repo,
        soul_repo: repos.soul_repo,
        user_profile_repo: repos.user_profile_repo,
        conversation_service: wired.conversation_service,
        goals_service: wired.goals_service,
        runtime_session: wired.runtime_session,
        screen_capture: wired.screen_capture,
        config_manager: wired.config_manager,
        mcp_tool_adapter: Some(wired.mcp_adapter),
        mcp_config_lock: Arc::new(tokio::sync::Mutex::new(())),
        mdns_announcer: infra.mdns_announcer,
        workers,
        memory_file,
    });

    Ok(state)
}

fn print_banner(config: &Config) {
    info!("═══════════════════════════════════════════════════════");
    info!("  BoBe Server Started");
    info!("  Engine: Copilot CLI (via github-copilot-sdk)");
    info!("  Capture enabled: {}", config.capture.enabled);
    info!("  Tools (MCP) enabled: {}", config.mcp.enabled);
    info!("═══════════════════════════════════════════════════════");
}
