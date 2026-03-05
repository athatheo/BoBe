//! Database pool creation and schema initialization.

use sqlx::Row;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use tracing::info;

use crate::error::AppError;

/// All statements use `IF NOT EXISTS` — safe to run on every startup.
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

    ensure_compat_schema(&pool).await?;

    info!("database.schema_applied");
    Ok(pool)
}

fn normalize_sqlite_url(db_url: &str) -> String {
    if let Some(path) = db_url.strip_prefix("sqlite:") {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        return format!("sqlite:{}", path.replace('~', &home));
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

async fn ensure_compat_schema(pool: &SqlitePool) -> Result<(), AppError> {
    ensure_column(pool, "memories", "source_observation_id", "BLOB").await?;
    ensure_column(pool, "memories", "source_conversation_id", "BLOB").await?;
    Ok(())
}

async fn ensure_column(
    pool: &SqlitePool,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<(), AppError> {
    let pragma_sql = format!("PRAGMA table_info({table})");
    let rows = sqlx::query(&pragma_sql)
        .fetch_all(pool)
        .await
        .map_err(AppError::Database)?;

    let has_column = rows.iter().any(|row| {
        row.try_get::<String, _>("name")
            .map(|name| name == column)
            .unwrap_or(false)
    });
    if has_column {
        return Ok(());
    }

    let alter_sql = format!("ALTER TABLE {table} ADD COLUMN {column} {definition}");
    sqlx::query(&alter_sql)
        .execute(pool)
        .await
        .map_err(AppError::Database)?;
    info!(table, column, "database.column_added");
    Ok(())
}
