//! Database pool creation and schema migration.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use tracing::info;

use crate::error::AppError;

/// Open (or create) the SQLite database and run pending migrations.
pub async fn connect_and_migrate(db_url: &str) -> Result<SqlitePool, AppError> {
    // Ensure parent directory exists for file-based SQLite.
    if let Some(path) = db_url.strip_prefix("sqlite:")
        && let Some(parent) = std::path::Path::new(path).parent()
    {
        tokio::fs::create_dir_all(parent).await?;
    }

    let opts: SqliteConnectOptions = db_url
        .parse::<SqliteConnectOptions>()
        .map_err(AppError::Database)?
        .create_if_missing(true)
        .pragma("journal_mode", "WAL")
        .pragma("foreign_keys", "ON")
        .pragma("busy_timeout", "5000");

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await
        .map_err(AppError::Database)?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|e| AppError::Database(e.into()))?;

    info!("database.migrations_complete");
    Ok(pool)
}
