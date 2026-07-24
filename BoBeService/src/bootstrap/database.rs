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
        .parse::<SqliteConnectOptions>()?
        .create_if_missing(true)
        .pragma("journal_mode", "WAL")
        .pragma("foreign_keys", "ON")
        .pragma("busy_timeout", "5000");

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await?;

    sqlx::raw_sql(SCHEMA).execute(&pool).await?;
    apply_additive_schema_migrations(&pool).await?;

    info!("database.schema_applied");
    Ok(pool)
}

async fn apply_additive_schema_migrations(pool: &SqlitePool) -> Result<(), AppError> {
    let has_completion_state: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('conversation_turns') WHERE name = 'is_complete'",
    )
    .fetch_one(pool)
    .await?;
    if has_completion_state == 0 {
        sqlx::query(
            "ALTER TABLE conversation_turns ADD COLUMN is_complete INTEGER NOT NULL DEFAULT 1",
        )
        .execute(pool)
        .await?;
    }
    Ok(())
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

#[cfg(test)]
#[allow(clippy::expect_used, reason = "tests panic on precondition failures")]
mod tests {
    use super::*;

    #[tokio::test]
    async fn additive_migration_preserves_existing_turns_as_complete() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect");
        sqlx::raw_sql(
            "CREATE TABLE conversation_turns (
                id BLOB PRIMARY KEY NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                conversation_id BLOB NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
             );
             INSERT INTO conversation_turns
                (id, role, content, conversation_id, created_at, updated_at)
             VALUES
                (x'01', 'assistant', 'existing answer', x'02', '2026-01-01', '2026-01-01');",
        )
        .execute(&pool)
        .await
        .expect("create legacy turn table");

        apply_additive_schema_migrations(&pool)
            .await
            .expect("apply additive migration");

        let is_complete: i64 = sqlx::query_scalar("SELECT is_complete FROM conversation_turns")
            .fetch_one(&pool)
            .await
            .expect("read migrated turn");
        assert_eq!(is_complete, 1);
    }
}
