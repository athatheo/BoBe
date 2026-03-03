//! Database pool creation and schema initialization.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use tracing::info;

use crate::error::AppError;

/// The full database schema. All statements use `IF NOT EXISTS` so this is
/// safe to run on every startup — no migration tracking needed.
const SCHEMA: &str = include_str!("../../migrations/schema.sql");

/// Open (or create) the SQLite database and apply the schema.
pub async fn connect_and_apply_schema(db_url: &str) -> Result<SqlitePool, AppError> {
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

    sqlx::raw_sql(SCHEMA)
        .execute(&pool)
        .await
        .map_err(AppError::Database)?;

    info!("database.schema_applied");
    Ok(pool)
}
