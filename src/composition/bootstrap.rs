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

    // 7. Build AppState from the container
    let state = Arc::new(AppState {
        db: container.db,
        config: container.config,
        http_client: container.http_client,
        event_queue: container.event_queue,
        connection_manager: container.connection_manager,
        llm_provider: container.llm_provider,
        embedding_provider: container.embedding_provider,
        soul_repo: container.soul_repo,
        user_profile_repo: container.user_profile_repo,
        screen_capture: container.screen_capture,
        ollama_manager: container.ollama_manager,
        config_manager: container.config_manager,
    });

    // 8. Print startup banner
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
