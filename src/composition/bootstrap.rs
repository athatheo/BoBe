use std::sync::Arc;

use sqlx::sqlite::SqlitePool;
use tracing::{error, info, warn};

use crate::app_state::AppState;
use crate::config::Config;
use crate::error::AppError;

use super::container::Container;
use super::db_seeding;

/// Run the full application bootstrap sequence.
///
/// 1. Create SQLite pool
/// 2. Run migrations
/// 3. Build the Container (wires all dependencies)
/// 4. Ensure Ollama is running (if backend is ollama)
/// 5. Seed default documents
/// 6. Build AppState
pub async fn run(config: Config) -> Result<Arc<AppState>, AppError> {
    // 1. Ensure data directory exists
    let db_url = &config.database_url;
    if let Some(path) = db_url.strip_prefix("sqlite:") {
        if let Some(parent) = std::path::Path::new(path).parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
    }

    // 2. Create pool and run migrations
    let pool = SqlitePool::connect(db_url)
        .await
        .map_err(AppError::Database)?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|e| AppError::Database(e.into()))?;
    info!("database.migrations_complete");

    // 3. Build the container (wires all concrete types)
    let container = Container::build(config.clone(), pool)?;

    // 4. Ensure Ollama is running (if using Ollama backend)
    if config.llm_backend == "ollama" || config.vision_backend == "ollama" {
        match container.ollama_manager.ensure_running().await {
            Ok(()) => info!(model = %config.ollama_model, "ollama.ready"),
            Err(e) => {
                error!(error = %e, "ollama.startup_failed");
                // Don't fail hard — LLM might become available later
                warn!("Continuing without Ollama — LLM calls will fail until it's available");
            }
        }

        // Also ensure vision model if needed
        if config.vision_backend == "ollama" {
            match container
                .ollama_manager
                .ensure_model(&config.vision_ollama_model)
                .await
            {
                Ok(true) => info!(model = %config.vision_ollama_model, "ollama.vision_ready"),
                Ok(false) => warn!(model = %config.vision_ollama_model, "ollama.vision_model_unavailable"),
                Err(e) => warn!(error = %e, "ollama.vision_model_check_failed"),
            }
        }
    }

    // 5. Seed default documents
    if config.seed_default_documents {
        if let Err(e) = db_seeding::seed_default_souls(container.soul_repo.as_ref()).await {
            warn!(error = %e, "bootstrap.soul_seeding_failed");
        }
        if let Err(e) =
            db_seeding::seed_default_user_profiles(container.user_profile_repo.as_ref()).await
        {
            warn!(error = %e, "bootstrap.user_profile_seeding_failed");
        }
    }

    // 6. Start mDNS if enabled
    container.mdns_announcer.start().await;

    // 7. Register native tools with the registry
    {
        container
            .tool_registry
            .register(container.native_adapter.clone() as Arc<dyn crate::ports::tools::ToolSource>)
            .await;
        info!(
            tools = container.native_adapter.tool_names().len(),
            "bootstrap.native_tools_registered"
        );
    }

    // 8. Mark orphaned agent jobs as failed
    {
        use crate::domain::types::AgentJobStatus;
        match container
            .agent_job_repo
            .find_by_status(AgentJobStatus::Running)
            .await
        {
            Ok(orphans) if !orphans.is_empty() => {
                info!(count = orphans.len(), "bootstrap.orphaned_jobs_found");
                // They will be picked up by the agent job trigger and re-evaluated.
                // For now just log — the trigger handles stale jobs.
            }
            Ok(_) => {}
            Err(e) => warn!(error = %e, "bootstrap.orphan_check_failed"),
        }
    }

    // 9. Sync goals from file if configured
    if config.goals_sync_on_startup {
        match container.goals_service.sync_from_file().await {
            Ok(result) => {
                info!(
                    created = result.created,
                    updated = result.updated,
                    "bootstrap.goals_synced_from_file"
                );
            }
            Err(e) => warn!(error = %e, "bootstrap.goals_sync_failed"),
        }
    }

    // 10. Build AppState from the container
    let state = Arc::new(AppState {
        db: container.db,
        config: container.config,
        http_client: container.http_client,
        event_queue: container.event_queue,
        connection_manager: container.connection_manager,
        llm_provider: container.llm_provider,
        vision_llm_provider: container.vision_llm_provider,
        embedding_provider: container.embedding_provider,
        conversation_repo: container.conversation_repo,
        memory_repo: container.memory_repo,
        goal_repo: container.goal_repo,
        observation_repo: container.observation_repo,
        cooldown_repo: container.cooldown_repo,
        learning_state_repo: container.learning_state_repo,
        agent_job_repo: container.agent_job_repo,
        mcp_config_repo: container.mcp_config_repo,
        soul_repo: container.soul_repo,
        user_profile_repo: container.user_profile_repo,
        conversation_service: container.conversation_service,
        context_assembler: container.context_assembler,
        goals_service: container.goals_service,
        tool_registry: container.tool_registry,
        runtime_session: container.runtime_session,
        learning_loop: container.learning_loop,
        screen_capture: container.screen_capture,
        ollama_manager: container.ollama_manager,
        config_manager: container.config_manager,
    });

    // 11. Print startup banner
    print_banner(&config);

    Ok(state)
}

fn print_banner(config: &Config) {
    info!("═══════════════════════════════════════════════════════");
    info!("  BoBe Server Started");
    info!("  LLM backend: {}", config.llm_backend);
    info!("  Model: {}", config.ollama_model);
    info!("  Capture enabled: {}", config.capture_enabled);
    info!("  Learning enabled: {}", config.learning_enabled);
    info!("  Tools enabled: {}", config.tools_enabled);
    info!("═══════════════════════════════════════════════════════");
}
