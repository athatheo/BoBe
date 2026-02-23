use std::sync::Arc;
use arc_swap::ArcSwap;
use sqlx::sqlite::SqlitePool;

use crate::config::Config;
use crate::adapters::sse::event_queue::EventQueue;
use crate::error::AppError;

/// Shared application state passed through Axum extractors.
pub struct AppState {
    pub db: SqlitePool,
    pub config: Arc<ArcSwap<Config>>,
    pub event_queue: Arc<EventQueue>,
}

impl AppState {
    pub async fn new(config: Config) -> Result<Arc<Self>, AppError> {
        // Ensure data directory exists
        let db_url = &config.database_url;
        if let Some(path) = db_url.strip_prefix("sqlite:") {
            if let Some(parent) = std::path::Path::new(path).parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }

        let pool = SqlitePool::connect(db_url)
            .await
            .map_err(AppError::Database)?;

        // Run migrations
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|e| AppError::Database(e.into()))?;

        let state = Arc::new(Self {
            db: pool,
            config: Arc::new(ArcSwap::from_pointee(config)),
            event_queue: Arc::new(EventQueue::new(100)),
        });

        Ok(state)
    }

    pub fn config(&self) -> arc_swap::Guard<Arc<Config>> {
        self.config.load()
    }
}
