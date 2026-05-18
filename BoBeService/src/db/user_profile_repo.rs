use crate::error::AppError;
use crate::models::ids::UserProfileId;
use crate::models::user_profile::UserProfile;
use sqlx::SqlitePool;
use tracing::{debug, info, warn};

pub(crate) struct SqliteUserProfileRepo {
    pool: SqlitePool,
}

impl SqliteUserProfileRepo {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

impl SqliteUserProfileRepo {
    pub(crate) async fn save(&self, profile: &UserProfile) -> Result<UserProfile, AppError> {
        sqlx::query(
            r"INSERT INTO user_profiles (id, name, content, enabled, is_default, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
               ON CONFLICT(id) DO UPDATE SET
                   name = excluded.name,
                   content = excluded.content,
                   enabled = excluded.enabled,
                   is_default = excluded.is_default,
                   updated_at = excluded.updated_at",
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

    pub(crate) async fn get_by_id(&self, id: UserProfileId) -> Result<Option<UserProfile>, AppError> {
        sqlx::query_as::<_, UserProfile>("SELECT * FROM user_profiles WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    pub(crate) async fn get_by_name(&self, name: &str) -> Result<Option<UserProfile>, AppError> {
        sqlx::query_as::<_, UserProfile>("SELECT * FROM user_profiles WHERE name = ?1")
            .bind(name)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    pub(crate) async fn find_enabled(&self) -> Result<Vec<UserProfile>, AppError> {
        sqlx::query_as::<_, UserProfile>("SELECT * FROM user_profiles WHERE enabled = 1")
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    pub(crate) async fn get_all(&self) -> Result<Vec<UserProfile>, AppError> {
        sqlx::query_as::<_, UserProfile>("SELECT * FROM user_profiles")
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    pub(crate) async fn update(
        &self,
        id: UserProfileId,
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

    pub(crate) async fn delete(&self, id: UserProfileId) -> Result<bool, AppError> {
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

#[cfg(test)]
#[allow(clippy::expect_used, reason = "tests panic on precondition failures")]
mod tests {
    use super::*;
    use crate::db::test_helpers::in_memory_pool;

    fn fixture(name: &str, enabled: bool) -> UserProfile {
        let mut p = UserProfile::new(name.to_owned(), format!("# {name}"), false);
        p.enabled = enabled;
        p
    }

    #[tokio::test]
    async fn save_then_get_by_name_roundtrip() {
        let pool = in_memory_pool().await;
        let repo = SqliteUserProfileRepo::new(pool);
        repo.save(&fixture("alice", true)).await.expect("save");
        let fetched = repo
            .get_by_name("alice")
            .await
            .expect("get_by_name")
            .expect("profile exists");
        assert_eq!(fetched.name, "alice");
        assert!(fetched.enabled);
    }

    #[tokio::test]
    async fn find_enabled_excludes_disabled() {
        let pool = in_memory_pool().await;
        let repo = SqliteUserProfileRepo::new(pool);
        repo.save(&fixture("a", true)).await.expect("save a");
        repo.save(&fixture("b", false)).await.expect("save b");
        let enabled = repo.find_enabled().await.expect("find_enabled");
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].name, "a");
    }

    #[tokio::test]
    async fn update_clears_enabled() {
        let pool = in_memory_pool().await;
        let repo = SqliteUserProfileRepo::new(pool);
        let saved = repo.save(&fixture("x", true)).await.expect("save");
        let updated = repo
            .update(saved.id, None, Some(false))
            .await
            .expect("update")
            .expect("returned profile");
        assert!(!updated.enabled);
    }

    #[tokio::test]
    async fn delete_existing_returns_true() {
        let pool = in_memory_pool().await;
        let repo = SqliteUserProfileRepo::new(pool);
        let p = repo.save(&fixture("doomed", true)).await.expect("save");
        assert!(repo.delete(p.id).await.expect("delete"));
        assert!(
            repo.get_by_id(p.id)
                .await
                .expect("get_by_id after delete")
                .is_none()
        );
    }
}
