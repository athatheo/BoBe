use async_trait::async_trait;
use sqlx::SqlitePool;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::models::soul::Soul;
use crate::error::AppError;
use crate::db::SoulRepository;

pub struct SqliteSoulRepo {
    pool: SqlitePool,
}

impl SqliteSoulRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SoulRepository for SqliteSoulRepo {
    async fn save(&self, soul: &Soul) -> Result<Soul, AppError> {
        sqlx::query(
            r#"INSERT INTO souls (id, name, content, enabled, is_default, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
               ON CONFLICT(id) DO UPDATE SET
                   name = excluded.name,
                   content = excluded.content,
                   enabled = excluded.enabled,
                   is_default = excluded.is_default,
                   updated_at = excluded.updated_at"#,
        )
        .bind(soul.id)
        .bind(&soul.name)
        .bind(&soul.content)
        .bind(soul.enabled)
        .bind(soul.is_default)
        .bind(soul.created_at)
        .bind(soul.updated_at)
        .execute(&self.pool)
        .await
        .map_err(AppError::Database)?;

        debug!(soul_id = %soul.id, name = %soul.name, is_default = soul.is_default, "soul_repo.saved");
        Ok(soul.clone())
    }

    async fn get_by_id(&self, id: Uuid) -> Result<Option<Soul>, AppError> {
        sqlx::query_as::<_, Soul>("SELECT * FROM souls WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn get_by_name(&self, name: &str) -> Result<Option<Soul>, AppError> {
        sqlx::query_as::<_, Soul>("SELECT * FROM souls WHERE name = ?1")
            .bind(name)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn get_default(&self) -> Result<Option<Soul>, AppError> {
        sqlx::query_as::<_, Soul>("SELECT * FROM souls WHERE is_default = 1 LIMIT 1")
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn get_all(&self) -> Result<Vec<Soul>, AppError> {
        sqlx::query_as::<_, Soul>("SELECT * FROM souls")
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn find_enabled(&self) -> Result<Vec<Soul>, AppError> {
        sqlx::query_as::<_, Soul>("SELECT * FROM souls WHERE enabled = 1")
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn update(
        &self,
        id: Uuid,
        content: Option<&str>,
        enabled: Option<bool>,
        is_default: Option<bool>,
        name: Option<&str>,
    ) -> Result<Option<Soul>, AppError> {
        let existing = self.get_by_id(id).await?;
        if existing.is_none() {
            warn!(soul_id = %id, "soul_repo.update.not_found");
            return Ok(None);
        }

        let mut sets = Vec::new();
        if content.is_some() {
            sets.push("content = ?");
        }
        if enabled.is_some() {
            sets.push("enabled = ?");
        }
        if is_default.is_some() {
            sets.push("is_default = ?");
        }
        if name.is_some() {
            sets.push("name = ?");
        }
        sets.push("updated_at = ?");

        let sql = format!("UPDATE souls SET {} WHERE id = ?", sets.join(", "));
        let mut q = sqlx::query(&sql);
        if let Some(c) = content {
            q = q.bind(c);
        }
        if let Some(e) = enabled {
            q = q.bind(e);
        }
        if let Some(d) = is_default {
            q = q.bind(d);
        }
        if let Some(n) = name {
            q = q.bind(n);
        }
        q = q.bind(chrono::Utc::now()).bind(id);
        q.execute(&self.pool).await.map_err(AppError::Database)?;

        info!(
            soul_id = %id,
            content_updated = content.is_some(),
            enabled = ?enabled,
            "soul_repo.updated"
        );
        self.get_by_id(id).await
    }

    async fn delete(&self, id: Uuid) -> Result<bool, AppError> {
        let result = sqlx::query("DELETE FROM souls WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(AppError::Database)?;

        if result.rows_affected() > 0 {
            info!(soul_id = %id, "soul_repo.deleted");
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
