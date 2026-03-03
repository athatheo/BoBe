//! Application bootstrap — wires all dependencies and starts background services.
//!
//! Split into focused submodules by lifecycle phase:
//! - `database`  — pool creation and migrations
//! - `infra`     — LLM/embedding providers, HTTP client, SSE, Ollama, mDNS
//! - `repos`     — repository trait object construction
//! - `wiring`    — services, tools, learners, triggers, runtime session assembly
//! - `integrity` — startup data-integrity checks (orphan cleanup, embedding repair)

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
use crate::services::goal_worker::manager::GoalWorkerManager;

/// Bootstrap the full application from a validated `Config`.
///
/// Returns the shared `AppState` (for Axum) and a `GoalWorkerManager`
/// (owned by `main` so it can be shut down independently).
pub async fn run(config: Config) -> Result<(Arc<AppState>, GoalWorkerManager), AppError> {
    let pool = database::connect_and_migrate(&config.database.url).await?;

    let infra = infra::Infrastructure::build(&config)?;
    let repos = repos::Repositories::from_pool(&pool);

    if config.mcp.enabled
        && let Err(e) =
            crate::tools::mcp::config::ensure_mcp_config_exists(config.mcp.config_file.as_deref())
    {
        warn!(error = %e, "bootstrap.ensure_mcp_config_failed");
    }

    let wired = wiring::wire(&config, &infra, &repos).await;

    integrity::run(&pool, repos.agent_job_repo.as_ref()).await;

    infra::ensure_ollama_ready(&config, &infra.ollama_manager).await;
    infra::detect_context_window(&config, &infra.config_arc, &infra.ollama_manager).await;

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

    if config.goals.sync_on_startup
        && let Err(e) = wired.goals_service.sync_from_file().await
    {
        tracing::warn!(error = %e, "bootstrap.goals_sync_failed");
    }

    wired.register_tools(&config, &infra.event_queue).await;

    wired.wire_sse_callbacks(&infra.connection_manager).await;

    infra.mdns_announcer.start().await;

    print_banner(&infra.config_arc.load());

    let state = Arc::new(AppState {
        db: pool,
        config: infra.config_arc.clone(),
        http_client: infra.http_client,
        event_queue: infra.event_queue,
        connection_manager: infra.connection_manager,
        llm_provider: infra.llm_provider,
        vision_llm_provider: infra.vision_llm_provider,
        embedding_provider: infra.embedding_provider,
        conversation_repo: repos.conversation_repo,
        memory_repo: repos.memory_repo,
        goal_repo: repos.goal_repo,
        observation_repo: repos.observation_repo,
        cooldown_repo: repos.cooldown_repo,
        learning_state_repo: repos.learning_state_repo,
        agent_job_repo: repos.agent_job_repo,
        soul_repo: repos.soul_repo,
        user_profile_repo: repos.user_profile_repo,
        goal_plan_repo: repos.goal_plan_repo,
        conversation_service: wired.conversation_service,
        context_assembler: wired.context_assembler,
        goals_service: wired.goals_service,
        tool_registry: wired.tool_registry,
        runtime_session: wired.runtime_session,
        learning_loop: wired.learning_loop,
        screen_capture: wired.screen_capture,
        ollama_manager: infra.ollama_manager,
        binary_manager: infra.binary_manager,
        config_manager: wired.config_manager,
        mcp_tool_adapter: Some(wired.mcp_adapter),
        mcp_config_lock: Arc::new(tokio::sync::Mutex::new(())),
        mdns_announcer: infra.mdns_announcer,
    });

    Ok((state, wired.goal_worker_manager))
}

fn print_banner(config: &Config) {
    info!("═══════════════════════════════════════════════════════");
    info!("  BoBe Server Started");
    info!("  LLM backend: {}", config.llm.backend);
    info!("  Model: {}", config.ollama.model);
    info!("  Context window: {} tokens", config.llm.context_window);
    info!("  Capture enabled: {}", config.capture.enabled);
    info!("  Learning enabled: {}", config.learning.enabled);
    info!("  Tools enabled: {}", config.tools.enabled);
    info!("═══════════════════════════════════════════════════════");
}
