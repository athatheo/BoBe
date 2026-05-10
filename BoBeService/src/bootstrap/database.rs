use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use tracing::info;

use crate::error::AppError;

const SCHEMA: &str = include_str!("../../migrations/schema.sql");

pub(crate) async fn connect_and_apply_schema(db_url: &str) -> Result<SqlitePool, AppError> {
    let db_url = normalize_sqlite_url(db_url);

    if let Some(path) = sqlite_file_path(&db_url)
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

fn normalize_sqlite_url(db_url: &str) -> String {
    if let Some(path) = db_url.strip_prefix("sqlite:") {
        let expanded = crate::util::paths::expand_tilde(path);
        return format!("sqlite:{}", expanded.display());
    }
    db_url.to_string()
}

fn sqlite_file_path(db_url: &str) -> Option<&str> {
    let raw = db_url.strip_prefix("sqlite:")?;
    if raw.starts_with(":memory:") {
        return None;
    }
    Some(raw.split('?').next().unwrap_or(raw))
}
