use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::db::ConversationRepository;
use crate::error::AppError;
use crate::models::conversation::{Conversation, ConversationTurn};
use crate::models::types::{ConversationState, TurnRole};

pub struct SqliteConversationRepo {
    pool: SqlitePool,
}

impl SqliteConversationRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ConversationRepository for SqliteConversationRepo {
    async fn save(&self, conversation: &Conversation) -> Result<Conversation, AppError> {
        sqlx::query(
            r#"INSERT INTO conversations (id, state, closed_at, summary, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6)
               ON CONFLICT(id) DO UPDATE SET
                   state = excluded.state,
                   closed_at = excluded.closed_at,
                   summary = excluded.summary,
                   updated_at = excluded.updated_at"#,
        )
        .bind(conversation.id)
        .bind(conversation.state)
        .bind(conversation.closed_at)
        .bind(&conversation.summary)
        .bind(conversation.created_at)
        .bind(conversation.updated_at)
        .execute(&self.pool)
        .await
        .map_err(AppError::Database)?;

        debug!(conversation_id = %conversation.id, state = %conversation.state, "conversation_repo.saved");
        Ok(conversation.clone())
    }

    async fn get_by_id(&self, id: Uuid) -> Result<Option<Conversation>, AppError> {
        let row = sqlx::query_as::<_, Conversation>("SELECT * FROM conversations WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)?;
        Ok(row)
    }

    async fn get_active(&self) -> Result<Option<Conversation>, AppError> {
        let row = sqlx::query_as::<_, Conversation>(
            "SELECT * FROM conversations WHERE state = ?1 LIMIT 1",
        )
        .bind(ConversationState::Active.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::Database)?;
        Ok(row)
    }

    async fn get_pending_or_active(&self) -> Result<Option<Conversation>, AppError> {
        let row = sqlx::query_as::<_, Conversation>(
            "SELECT * FROM conversations WHERE state IN (?1, ?2) ORDER BY updated_at DESC LIMIT 1",
        )
        .bind(ConversationState::Pending.as_str())
        .bind(ConversationState::Active.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::Database)?;
        Ok(row)
    }

    async fn find_by_state(
        &self,
        state: ConversationState,
        limit: i64,
    ) -> Result<Vec<Conversation>, AppError> {
        let rows = sqlx::query_as::<_, Conversation>(
            "SELECT * FROM conversations WHERE state = ?1 ORDER BY created_at DESC LIMIT ?2",
        )
        .bind(state.as_str())
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::Database)?;
        Ok(rows)
    }

    async fn find_recent(&self, limit: i64) -> Result<Vec<Conversation>, AppError> {
        let rows = sqlx::query_as::<_, Conversation>(
            "SELECT * FROM conversations ORDER BY created_at DESC LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::Database)?;
        Ok(rows)
    }

    async fn find_closed_since(
        &self,
        since: Option<DateTime<Utc>>,
    ) -> Result<Vec<Conversation>, AppError> {
        let rows = if let Some(since) = since {
            sqlx::query_as::<_, Conversation>(
                "SELECT * FROM conversations WHERE state = ?1 AND closed_at > ?2 ORDER BY closed_at ASC",
            )
            .bind(ConversationState::Closed.as_str())
            .bind(since)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as::<_, Conversation>(
                "SELECT * FROM conversations WHERE state = ?1 ORDER BY closed_at ASC",
            )
            .bind(ConversationState::Closed.as_str())
            .fetch_all(&self.pool)
            .await
        }
        .map_err(AppError::Database)?;

        debug!(since = ?since, count = rows.len(), "conversation_repo.find_closed_since");
        Ok(rows)
    }

    async fn get_last_closed(&self) -> Result<Option<Conversation>, AppError> {
        let row = sqlx::query_as::<_, Conversation>(
            "SELECT * FROM conversations WHERE state = ?1 ORDER BY closed_at DESC LIMIT 1",
        )
        .bind(ConversationState::Closed.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::Database)?;
        Ok(row)
    }

    async fn update_state(
        &self,
        id: Uuid,
        state: ConversationState,
        summary: Option<String>,
    ) -> Result<Option<Conversation>, AppError> {
        let now = Utc::now();
        let closed_at = if state == ConversationState::Closed {
            Some(now)
        } else {
            None
        };

        let result = sqlx::query(
            r#"UPDATE conversations SET state = ?1, summary = COALESCE(?2, summary),
               closed_at = COALESCE(?3, closed_at), updated_at = ?4 WHERE id = ?5"#,
        )
        .bind(state.as_str())
        .bind(&summary)
        .bind(closed_at)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(AppError::Database)?;

        if result.rows_affected() == 0 {
            warn!(conversation_id = %id, "conversation_repo.update_state.not_found");
            return Ok(None);
        }

        info!(
            conversation_id = %id,
            to_state = %state.as_str(),
            has_summary = summary.is_some(),
            "conversation_repo.updated_state"
        );
        self.get_by_id(id).await
    }

    async fn add_turn(&self, turn: &ConversationTurn) -> Result<ConversationTurn, AppError> {
        // Atomic: verify not closed + insert turn + touch timestamp
        let mut tx = self.pool.begin().await.map_err(AppError::Database)?;

        // Check conversation state inside transaction to prevent TOCTOU race
        let conv_state: Option<(String,)> =
            sqlx::query_as("SELECT state FROM conversations WHERE id = ?1")
                .bind(turn.conversation_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(AppError::Database)?;

        match conv_state {
            None => {
                warn!(conversation_id = %turn.conversation_id, "conversation_repo.add_turn_not_found");
                return Err(AppError::NotFound(format!(
                    "Conversation {} not found",
                    turn.conversation_id
                )));
            }
            Some((state,)) if state == "closed" => {
                warn!(conversation_id = %turn.conversation_id, role = %turn.role, "conversation_repo.add_turn_closed");
                return Err(AppError::Validation(format!(
                    "Cannot add turn to closed conversation {}",
                    turn.conversation_id
                )));
            }
            _ => {}
        }

        sqlx::query(
            r#"INSERT INTO conversation_turns (id, role, content, conversation_id, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6)"#,
        )
        .bind(turn.id)
        .bind(turn.role)
        .bind(&turn.content)
        .bind(turn.conversation_id)
        .bind(turn.created_at)
        .bind(turn.updated_at)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Database)?;

        sqlx::query("UPDATE conversations SET updated_at = ?1 WHERE id = ?2")
            .bind(Utc::now())
            .bind(turn.conversation_id)
            .execute(&mut *tx)
            .await
            .map_err(AppError::Database)?;

        tx.commit().await.map_err(AppError::Database)?;

        debug!(
            conversation_id = %turn.conversation_id,
            turn_id = %turn.id,
            role = %turn.role,
            content_length = turn.content.len(),
            "conversation_repo.add_turn"
        );
        Ok(turn.clone())
    }

    async fn get_turns(
        &self,
        conversation_id: Uuid,
        limit: i64,
    ) -> Result<Vec<ConversationTurn>, AppError> {
        let rows = sqlx::query_as::<_, ConversationTurn>(
            "SELECT * FROM conversation_turns WHERE conversation_id = ?1 ORDER BY created_at ASC LIMIT ?2",
        )
        .bind(conversation_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::Database)?;
        Ok(rows)
    }

    async fn get_recent_turns_by_role(
        &self,
        role: TurnRole,
        limit: i64,
    ) -> Result<Vec<String>, AppError> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT content FROM conversation_turns WHERE role = ?1 ORDER BY created_at DESC LIMIT ?2",
        )
        .bind(role.as_str())
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::Database)?;
        Ok(rows.into_iter().map(|(c,)| c).collect())
    }

    async fn delete(&self, id: Uuid) -> Result<bool, AppError> {
        // Turns are CASCADE deleted by FK
        let result = sqlx::query("DELETE FROM conversations WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(AppError::Database)?;

        if result.rows_affected() > 0 {
            info!(conversation_id = %id, "conversation_repo.deleted");
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
