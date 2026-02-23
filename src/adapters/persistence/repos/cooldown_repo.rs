use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use tokio::sync::Mutex;
use tracing::{debug, info};
use uuid::Uuid;

use crate::domain::cooldown::{Cooldown, CooldownInfo};
use crate::error::AppError;
use crate::ports::repos::cooldown_repo::CooldownRepository;

/// SQLite cooldown repository.
///
/// Manages a single-row cooldown_state table with an in-memory cache.
/// Uses tokio::sync::Mutex since the guard must be held across .await.
pub struct SqliteCooldownRepo {
    pool: SqlitePool,
    state: Mutex<Option<Cooldown>>,
}

impl SqliteCooldownRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            state: Mutex::new(None),
        }
    }

    async fn ensure_loaded(&self) -> Result<Uuid, AppError> {
        let guard = self.state.lock().await;
        if let Some(s) = guard.as_ref() {
            return Ok(s.id);
        }
        drop(guard);
        self.load_or_create().await?;
        let guard = self.state.lock().await;
        Ok(guard.as_ref().map(|s| s.id).unwrap_or_else(Uuid::new_v4))
    }
}

#[async_trait]
impl CooldownRepository for SqliteCooldownRepo {
    fn last_engagement(&self) -> Option<DateTime<Utc>> {
        self.state.try_lock().ok().and_then(|s| s.as_ref()?.last_engagement)
    }

    fn last_user_response(&self) -> Option<DateTime<Utc>> {
        self.state.try_lock().ok().and_then(|s| s.as_ref()?.last_user_response)
    }

    fn check_cooldown(&self, base_minutes: i64, extended_minutes: i64) -> Option<CooldownInfo> {
        self.state
            .try_lock()
            .ok()
            .and_then(|s| s.as_ref()?.check_cooldown(base_minutes, extended_minutes))
    }

    async fn load_or_create(&self) -> Result<(), AppError> {
        let row = sqlx::query_as::<_, Cooldown>(
            "SELECT * FROM cooldown_state LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::Database)?;

        let state = if let Some(existing) = row {
            info!(
                state_id = %existing.id,
                last_engagement = ?existing.last_engagement,
                last_user_response = ?existing.last_user_response,
                "cooldown_repository.state_loaded"
            );
            existing
        } else {
            let new_state = Cooldown::new();
            let id = new_state.id.to_string();
            sqlx::query(
                "INSERT INTO cooldown_state (id, created_at, updated_at) VALUES (?1, ?2, ?3)",
            )
            .bind(&id)
            .bind(&new_state.created_at)
            .bind(&new_state.updated_at)
            .execute(&self.pool)
            .await
            .map_err(AppError::Database)?;
            info!(state_id = %id, "cooldown_repository.state_created");
            new_state
        };

        let mut guard = self.state.lock().await;
        *guard = Some(state);
        Ok(())
    }

    async fn update_last_engagement(&self, timestamp: DateTime<Utc>) -> Result<(), AppError> {
        let id = self.ensure_loaded().await?;

        sqlx::query(
            "UPDATE cooldown_state SET last_engagement = ?1, updated_at = ?2 WHERE id = ?3",
        )
        .bind(&timestamp)
        .bind(&Utc::now())
        .bind(id.to_string())
        .execute(&self.pool)
        .await
        .map_err(AppError::Database)?;

        let mut guard = self.state.lock().await;
        if let Some(s) = guard.as_mut() {
            s.last_engagement = Some(timestamp);
        }

        debug!(last_engagement = %timestamp, "cooldown_repository.state_saved");
        Ok(())
    }

    async fn update_last_user_response(&self, timestamp: DateTime<Utc>) -> Result<(), AppError> {
        let id = self.ensure_loaded().await?;

        sqlx::query(
            "UPDATE cooldown_state SET last_user_response = ?1, updated_at = ?2 WHERE id = ?3",
        )
        .bind(&timestamp)
        .bind(&Utc::now())
        .bind(id.to_string())
        .execute(&self.pool)
        .await
        .map_err(AppError::Database)?;

        let mut guard = self.state.lock().await;
        if let Some(s) = guard.as_mut() {
            s.last_user_response = Some(timestamp);
        }

        debug!(last_user_response = %timestamp, "cooldown_repository.state_saved");
        Ok(())
    }
}
