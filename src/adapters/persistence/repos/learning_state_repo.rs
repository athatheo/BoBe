use async_trait::async_trait;
use sqlx::SqlitePool;
use tracing::{debug, info};

use crate::domain::learning_state::LearningState;
use crate::error::AppError;
use crate::ports::repos::learning_state_repo::LearningStateRepository;

pub struct SqliteLearningStateRepo {
    pool: SqlitePool,
}

impl SqliteLearningStateRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl LearningStateRepository for SqliteLearningStateRepo {
    async fn get_or_create(&self) -> Result<LearningState, AppError> {
        let row = sqlx::query_as::<_, LearningState>("SELECT * FROM learning_state LIMIT 1")
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)?;

        if let Some(state) = row {
            info!(
                state_id = %state.id,
                last_conversation = ?state.last_conversation_processed_at,
                last_context = ?state.last_context_processed_at,
                last_consolidation = ?state.last_consolidation_at,
                last_pruning = ?state.last_pruning_at,
                "learning_state_repo.loaded"
            );
            return Ok(state);
        }

        let new_state = LearningState::new();
        sqlx::query("INSERT INTO learning_state (id, created_at, updated_at) VALUES (?1, ?2, ?3)")
            .bind(new_state.id)
            .bind(new_state.created_at)
            .bind(new_state.updated_at)
            .execute(&self.pool)
            .await
            .map_err(AppError::Database)?;

        info!(state_id = %new_state.id, "learning_state_repo.created");
        Ok(new_state)
    }

    async fn save(&self, state: &LearningState) -> Result<(), AppError> {
        sqlx::query(
            r#"UPDATE learning_state SET
                   last_conversation_processed_at = ?1,
                   last_context_processed_at = ?2,
                   last_consolidation_at = ?3,
                   last_pruning_at = ?4,
                   updated_at = ?5
               WHERE id = ?6"#,
        )
        .bind(state.last_conversation_processed_at)
        .bind(state.last_context_processed_at)
        .bind(state.last_consolidation_at)
        .bind(state.last_pruning_at)
        .bind(chrono::Utc::now())
        .bind(state.id)
        .execute(&self.pool)
        .await
        .map_err(AppError::Database)?;

        debug!(
            last_conversation = ?state.last_conversation_processed_at,
            last_context = ?state.last_context_processed_at,
            last_consolidation = ?state.last_consolidation_at,
            last_pruning = ?state.last_pruning_at,
            "learning_state_repo.saved"
        );
        Ok(())
    }
}
