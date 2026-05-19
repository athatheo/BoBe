use crate::error::AppError;
use crate::models::conversation::{Conversation, ConversationTurn};
use crate::models::ids::{ConversationId, ConversationTurnId};
use crate::models::types::{ConversationState, TurnRole};
use chrono::Utc;
use sqlx::SqlitePool;
use tracing::{debug, info, warn};

pub(crate) struct SqliteConversationRepo {
    pool: SqlitePool,
}

impl SqliteConversationRepo {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub(crate) async fn save(&self, conversation: &Conversation) -> Result<Conversation, AppError> {
        sqlx::query(
            r"INSERT INTO conversations (id, state, closed_at, summary, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6)
               ON CONFLICT(id) DO UPDATE SET
                   state = excluded.state,
                   closed_at = excluded.closed_at,
                   summary = excluded.summary,
                   updated_at = excluded.updated_at",
        )
        .bind(conversation.id)
        .bind(conversation.state)
        .bind(conversation.closed_at)
        .bind(&conversation.summary)
        .bind(conversation.created_at)
        .bind(conversation.updated_at)
        .execute(&self.pool)
        .await?;

        debug!(conversation_id = %conversation.id, state = %conversation.state, "conversation_repo.saved");
        Ok(conversation.clone())
    }

    pub(crate) async fn get_by_id(
        &self,
        id: ConversationId,
    ) -> Result<Option<Conversation>, AppError> {
        Ok(
            sqlx::query_as::<_, Conversation>("SELECT * FROM conversations WHERE id = ?1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub(crate) async fn get_pending_or_active(&self) -> Result<Option<Conversation>, AppError> {
        Ok(sqlx::query_as::<_, Conversation>(
            "SELECT * FROM conversations WHERE state IN (?1, ?2) ORDER BY updated_at DESC LIMIT 1",
        )
        .bind(ConversationState::Pending.as_str())
        .bind(ConversationState::Active.as_str())
        .fetch_optional(&self.pool)
        .await?)
    }

    pub(crate) async fn get_last_closed(&self) -> Result<Option<Conversation>, AppError> {
        Ok(sqlx::query_as::<_, Conversation>(
            "SELECT * FROM conversations WHERE state = ?1 ORDER BY closed_at DESC LIMIT 1",
        )
        .bind(ConversationState::Closed.as_str())
        .fetch_optional(&self.pool)
        .await?)
    }

    pub(crate) async fn update_state(
        &self,
        id: ConversationId,
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
            r"UPDATE conversations SET state = ?1, summary = COALESCE(?2, summary),
               closed_at = COALESCE(?3, closed_at), updated_at = ?4 WHERE id = ?5",
        )
        .bind(state.as_str())
        .bind(&summary)
        .bind(closed_at)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await?;

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

    pub(crate) async fn add_turn(
        &self,
        turn: &ConversationTurn,
    ) -> Result<ConversationTurn, AppError> {
        // Transaction prevents TOCTOU race: verify not closed + insert + touch timestamp
        let mut tx = self.pool.begin().await?;

        let conv_state: Option<(String,)> =
            sqlx::query_as("SELECT state FROM conversations WHERE id = ?1")
                .bind(turn.conversation_id)
                .fetch_optional(&mut *tx)
                .await?;

        match conv_state {
            None => {
                warn!(conversation_id = %turn.conversation_id, "conversation_repo.add_turn_not_found");
                return Err(AppError::NotFound(format!(
                    "Conversation {} not found",
                    turn.conversation_id
                )));
            }
            Some((state,)) if state == ConversationState::Closed.as_str() => {
                warn!(conversation_id = %turn.conversation_id, role = %turn.role, "conversation_repo.add_turn_closed");
                return Err(AppError::Validation(format!(
                    "Cannot add turn to closed conversation {}",
                    turn.conversation_id
                )));
            }
            _ => {}
        }

        sqlx::query(
            r"INSERT INTO conversation_turns (id, role, content, conversation_id, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(turn.id)
        .bind(turn.role)
        .bind(&turn.content)
        .bind(turn.conversation_id)
        .bind(turn.created_at)
        .bind(turn.updated_at)
        .execute(&mut *tx)
        .await?;

        sqlx::query("UPDATE conversations SET updated_at = ?1 WHERE id = ?2")
            .bind(Utc::now())
            .bind(turn.conversation_id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        debug!(
            conversation_id = %turn.conversation_id,
            turn_id = %turn.id,
            role = %turn.role,
            content_length = turn.content.len(),
            "conversation_repo.add_turn"
        );
        Ok(turn.clone())
    }

    pub(crate) async fn update_turn_content(
        &self,
        turn_id: ConversationTurnId,
        content: &str,
    ) -> Result<Option<ConversationTurn>, AppError> {
        let mut tx = self.pool.begin().await?;
        let now = Utc::now();

        let conversation_id: Option<(ConversationId,)> =
            sqlx::query_as("SELECT conversation_id FROM conversation_turns WHERE id = ?1")
                .bind(turn_id)
                .fetch_optional(&mut *tx)
                .await?;

        let Some((conversation_id,)) = conversation_id else {
            warn!(turn_id = %turn_id, "conversation_repo.update_turn_content.not_found");
            return Ok(None);
        };

        sqlx::query(r"UPDATE conversation_turns SET content = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(content)
            .bind(now)
            .bind(turn_id)
            .execute(&mut *tx)
            .await?;

        sqlx::query("UPDATE conversations SET updated_at = ?1 WHERE id = ?2")
            .bind(now)
            .bind(conversation_id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        Ok(
            sqlx::query_as::<_, ConversationTurn>("SELECT * FROM conversation_turns WHERE id = ?1")
                .bind(turn_id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub(crate) async fn get_turns(
        &self,
        conversation_id: ConversationId,
        limit: i64,
    ) -> Result<Vec<ConversationTurn>, AppError> {
        Ok(sqlx::query_as::<_, ConversationTurn>(
            "SELECT * FROM conversation_turns WHERE conversation_id = ?1 ORDER BY created_at ASC LIMIT ?2",
        )
        .bind(conversation_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?)
    }

    /// Single-row staleness check — avoids loading 100 turn rows just to
    /// find the most-recent user turn. Returns `None` when no user turn
    /// has been recorded yet (the conversation `created_at` is then the
    /// staleness reference).
    pub(crate) async fn last_user_turn_at(
        &self,
        conversation_id: ConversationId,
    ) -> Result<Option<chrono::DateTime<Utc>>, AppError> {
        let row: Option<(Option<chrono::DateTime<Utc>>,)> = sqlx::query_as(
            "SELECT MAX(created_at) FROM conversation_turns WHERE conversation_id = ?1 AND role = ?2",
        )
        .bind(conversation_id)
        .bind(TurnRole::User.as_str())
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.and_then(|(ts,)| ts))
    }

    pub(crate) async fn count_turns(
        &self,
        conversation_id: ConversationId,
    ) -> Result<i64, AppError> {
        let row: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM conversation_turns WHERE conversation_id = ?1")
                .bind(conversation_id)
                .fetch_one(&self.pool)
                .await?;
        Ok(row.0)
    }

    pub(crate) async fn get_recent_turns_by_role(
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
        .await?;
        Ok(rows.into_iter().map(|(c,)| c).collect())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, reason = "tests panic on precondition failures")]
mod tests {
    use super::*;
    use crate::db::test_helpers::in_memory_pool;

    #[tokio::test]
    async fn save_then_get_by_id_roundtrip() {
        let pool = in_memory_pool().await;
        let repo = SqliteConversationRepo::new(pool);
        let conv = Conversation::new_pending();
        let saved = repo.save(&conv).await.expect("save");
        let fetched = repo
            .get_by_id(saved.id)
            .await
            .expect("get_by_id")
            .expect("conversation exists");
        assert_eq!(fetched.id, saved.id);
        assert_eq!(fetched.state, ConversationState::Pending);
    }

    #[tokio::test]
    async fn pending_lookup_finds_pending() {
        let pool = in_memory_pool().await;
        let repo = SqliteConversationRepo::new(pool);
        let pending = repo
            .save(&Conversation::new_pending())
            .await
            .expect("save pending");
        let found = repo
            .get_pending_or_active()
            .await
            .expect("get_pending_or_active")
            .expect("found pending");
        assert_eq!(found.id, pending.id);
        assert_eq!(found.state, ConversationState::Pending);
    }

    #[tokio::test]
    async fn update_state_transitions_pending_to_active() {
        let pool = in_memory_pool().await;
        let repo = SqliteConversationRepo::new(pool);
        let conv = repo.save(&Conversation::new_pending()).await.expect("save");
        let updated = repo
            .update_state(conv.id, ConversationState::Active, None)
            .await
            .expect("update_state")
            .expect("conversation found");
        assert_eq!(updated.state, ConversationState::Active);
    }

    #[tokio::test]
    async fn closed_lookup_returns_most_recent_closed() {
        let pool = in_memory_pool().await;
        let repo = SqliteConversationRepo::new(pool);
        let c = repo.save(&Conversation::new_active()).await.expect("save");
        repo.update_state(c.id, ConversationState::Closed, Some("summary".into()))
            .await
            .expect("close");
        let last = repo
            .get_last_closed()
            .await
            .expect("get_last_closed")
            .expect("has closed conversation");
        assert_eq!(last.id, c.id);
        assert_eq!(last.summary.as_deref(), Some("summary"));
    }

    #[tokio::test]
    async fn add_turn_and_get_turns_in_order() {
        let pool = in_memory_pool().await;
        let repo = SqliteConversationRepo::new(pool);
        let conv = repo.save(&Conversation::new_active()).await.expect("save");
        let t1 = ConversationTurn::new(conv.id, TurnRole::User, "hello".into());
        let t2 = ConversationTurn::new(conv.id, TurnRole::Assistant, "world".into());
        repo.add_turn(&t1).await.expect("add turn 1");
        repo.add_turn(&t2).await.expect("add turn 2");
        let turns = repo.get_turns(conv.id, 10).await.expect("get_turns");
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].content, "hello");
        assert_eq!(turns[1].content, "world");
    }

    #[tokio::test]
    async fn update_turn_content_overwrites() {
        let pool = in_memory_pool().await;
        let repo = SqliteConversationRepo::new(pool);
        let conv = repo.save(&Conversation::new_active()).await.expect("save");
        let turn = ConversationTurn::new(conv.id, TurnRole::Assistant, "draft".into());
        repo.add_turn(&turn).await.expect("add turn");
        let updated = repo
            .update_turn_content(turn.id, "final")
            .await
            .expect("update_turn_content")
            .expect("turn exists");
        assert_eq!(updated.content, "final");
    }
}
