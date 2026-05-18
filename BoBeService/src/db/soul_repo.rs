use crate::error::AppError;
use crate::models::ids::SoulId;
use crate::models::soul::Soul;
use sqlx::SqlitePool;
use tracing::{debug, info, warn};

pub(crate) struct SqliteSoulRepo {
    pool: SqlitePool,
}

impl SqliteSoulRepo {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

impl SqliteSoulRepo {
    pub(crate) async fn save(&self, soul: &Soul) -> Result<Soul, AppError> {
        sqlx::query(
            r"INSERT INTO souls (id, name, content, enabled, is_default, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
               ON CONFLICT(id) DO UPDATE SET
                   name = excluded.name,
                   content = excluded.content,
                   enabled = excluded.enabled,
                   is_default = excluded.is_default,
                   updated_at = excluded.updated_at",
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

    pub(crate) async fn get_by_id(&self, id: SoulId) -> Result<Option<Soul>, AppError> {
        sqlx::query_as::<_, Soul>("SELECT * FROM souls WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    pub(crate) async fn get_by_name(&self, name: &str) -> Result<Option<Soul>, AppError> {
        sqlx::query_as::<_, Soul>("SELECT * FROM souls WHERE name = ?1")
            .bind(name)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    pub(crate) async fn get_all(&self) -> Result<Vec<Soul>, AppError> {
        sqlx::query_as::<_, Soul>("SELECT * FROM souls")
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    pub(crate) async fn find_enabled(&self) -> Result<Vec<Soul>, AppError> {
        sqlx::query_as::<_, Soul>("SELECT * FROM souls WHERE enabled = 1")
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    pub(crate) async fn update(
        &self,
        id: SoulId,
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

    pub(crate) async fn delete(&self, id: SoulId) -> Result<bool, AppError> {
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

#[cfg(test)]
#[allow(clippy::expect_used, reason = "tests panic on precondition failures")]
mod tests {
    use super::*;
    use crate::db::test_helpers::in_memory_pool;
    use crate::models::soul::Soul;

    fn fixture(name: &str, enabled: bool) -> Soul {
        let mut s = Soul::new(name.to_owned(), format!("# {name}"), false);
        s.enabled = enabled;
        s
    }

    #[tokio::test]
    async fn save_then_get_by_id_roundtrip() {
        let pool = in_memory_pool().await;
        let repo = SqliteSoulRepo::new(pool);
        let soul = fixture("Test soul", true);
        let saved = repo.save(&soul).await.expect("save");
        let fetched = repo
            .get_by_id(saved.id)
            .await
            .expect("get_by_id")
            .expect("soul exists");
        assert_eq!(fetched.id, saved.id);
        assert_eq!(fetched.name, "Test soul");
        assert!(fetched.enabled);
    }

    #[tokio::test]
    async fn get_by_name_returns_none_when_absent() {
        let pool = in_memory_pool().await;
        let repo = SqliteSoulRepo::new(pool);
        assert!(
            repo.get_by_name("does-not-exist")
                .await
                .expect("get_by_name")
                .is_none()
        );
    }

    #[tokio::test]
    async fn find_enabled_excludes_disabled() {
        let pool = in_memory_pool().await;
        let repo = SqliteSoulRepo::new(pool);
        repo.save(&fixture("on", true)).await.expect("save on");
        repo.save(&fixture("off", false)).await.expect("save off");
        let enabled = repo.find_enabled().await.expect("find_enabled");
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].name, "on");
    }

    #[tokio::test]
    async fn update_changes_content_and_enabled() {
        let pool = in_memory_pool().await;
        let repo = SqliteSoulRepo::new(pool);
        let soul = repo.save(&fixture("x", true)).await.expect("save");
        let updated = repo
            .update(soul.id, Some("# new"), Some(false), None, None)
            .await
            .expect("update")
            .expect("returned soul");
        assert_eq!(updated.content, "# new");
        assert!(!updated.enabled);
    }

    #[tokio::test]
    async fn delete_returns_false_for_missing() {
        let pool = in_memory_pool().await;
        let repo = SqliteSoulRepo::new(pool);
        let missing = SoulId::new();
        assert!(!repo.delete(missing).await.expect("delete missing"));
    }

    #[tokio::test]
    async fn delete_returns_true_for_existing() {
        let pool = in_memory_pool().await;
        let repo = SqliteSoulRepo::new(pool);
        let soul = repo.save(&fixture("doomed", true)).await.expect("save");
        assert!(repo.delete(soul.id).await.expect("delete existing"));
        assert!(
            repo.get_by_id(soul.id)
                .await
                .expect("get_by_id after delete")
                .is_none()
        );
    }
}
