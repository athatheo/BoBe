use async_trait::async_trait;
use sqlx::SqlitePool;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::db::UserProfileRepository;
use crate::error::AppError;
use crate::models::user_profile::UserProfile;

pub struct SqliteUserProfileRepo {
    pool: SqlitePool,
}

impl SqliteUserProfileRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserProfileRepository for SqliteUserProfileRepo {
    async fn save(&self, profile: &UserProfile) -> Result<UserProfile, AppError> {
        sqlx::query(
            r#"INSERT INTO user_profiles (id, name, content, enabled, is_default, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
               ON CONFLICT(id) DO UPDATE SET
                   name = excluded.name,
                   content = excluded.content,
                   enabled = excluded.enabled,
                   is_default = excluded.is_default,
                   updated_at = excluded.updated_at"#,
        )
        .bind(profile.id)
        .bind(&profile.name)
        .bind(&profile.content)
        .bind(profile.enabled)
        .bind(profile.is_default)
        .bind(profile.created_at)
        .bind(profile.updated_at)
        .execute(&self.pool)
        .await
        .map_err(AppError::Database)?;

        debug!(profile_id = %profile.id, name = %profile.name, is_default = profile.is_default, "user_profile_repo.saved");
        Ok(profile.clone())
    }

    async fn get_by_id(&self, id: Uuid) -> Result<Option<UserProfile>, AppError> {
        sqlx::query_as::<_, UserProfile>("SELECT * FROM user_profiles WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn get_by_name(&self, name: &str) -> Result<Option<UserProfile>, AppError> {
        sqlx::query_as::<_, UserProfile>("SELECT * FROM user_profiles WHERE name = ?1")
            .bind(name)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn get_default(&self) -> Result<Option<UserProfile>, AppError> {
        sqlx::query_as::<_, UserProfile>("SELECT * FROM user_profiles WHERE is_default = 1 LIMIT 1")
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn find_enabled(&self) -> Result<Vec<UserProfile>, AppError> {
        sqlx::query_as::<_, UserProfile>("SELECT * FROM user_profiles WHERE enabled = 1")
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn get_all(&self) -> Result<Vec<UserProfile>, AppError> {
        sqlx::query_as::<_, UserProfile>("SELECT * FROM user_profiles")
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn update(
        &self,
        id: Uuid,
        content: Option<&str>,
        enabled: Option<bool>,
    ) -> Result<Option<UserProfile>, AppError> {
        let existing = self.get_by_id(id).await?;
        if existing.is_none() {
            warn!(profile_id = %id, "user_profile_repo.update.not_found");
            return Ok(None);
        }

        let mut sets = Vec::new();
        if content.is_some() {
            sets.push("content = ?");
        }
        if enabled.is_some() {
            sets.push("enabled = ?");
        }
        sets.push("updated_at = ?");

        let sql = format!("UPDATE user_profiles SET {} WHERE id = ?", sets.join(", "));
        let mut q = sqlx::query(&sql);
        if let Some(c) = content {
            q = q.bind(c);
        }
        if let Some(e) = enabled {
            q = q.bind(e);
        }
        q = q.bind(chrono::Utc::now()).bind(id);
        q.execute(&self.pool).await.map_err(AppError::Database)?;

        info!(
            profile_id = %id,
            content_updated = content.is_some(),
            enabled = ?enabled,
            "user_profile_repo.updated"
        );
        self.get_by_id(id).await
    }

    async fn delete(&self, id: Uuid) -> Result<bool, AppError> {
        let result = sqlx::query("DELETE FROM user_profiles WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(AppError::Database)?;

        if result.rows_affected() > 0 {
            info!(profile_id = %id, "user_profile_repo.deleted");
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
