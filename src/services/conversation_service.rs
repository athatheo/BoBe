//! ConversationService — manages conversation lifecycle and turn management.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::db::ConversationRepository;
use crate::error::AppError;
use crate::models::conversation::{Conversation, ConversationTurn};
use crate::models::types::{ConversationState, TurnRole};

pub(crate) struct ConversationService {
    repo: Arc<dyn ConversationRepository>,
}

impl ConversationService {
    pub(crate) fn new(repo: Arc<dyn ConversationRepository>) -> Self {
        Self { repo }
    }

    // ── Conversation Lifecycle ──────────────────────────────────────────

    pub(crate) async fn create_pending(&self, ai_message: &str) -> Result<Conversation, AppError> {
        let conversation = Conversation::new_pending();
        let saved = self.repo.save(&conversation).await?;

        let turn = ConversationTurn::new(saved.id, TurnRole::Assistant, ai_message.to_owned());
        self.repo.add_turn(&turn).await?;

        info!(
            conversation_id = %saved.id,
            state = %saved.state,
            "conversation.created_pending"
        );
        Ok(saved)
    }

    pub(crate) async fn create_active(&self, user_message: &str) -> Result<Conversation, AppError> {
        let conversation = Conversation::new_active();
        let saved = self.repo.save(&conversation).await?;

        let turn = ConversationTurn::new(saved.id, TurnRole::User, user_message.to_owned());
        self.repo.add_turn(&turn).await?;

        info!(
            conversation_id = %saved.id,
            state = %saved.state,
            "conversation.created_active"
        );
        Ok(saved)
    }

    pub(crate) async fn activate(
        &self,
        conversation_id: Uuid,
    ) -> Result<Option<Conversation>, AppError> {
        let conversation = self.repo.get_by_id(conversation_id).await?;
        let Some(conversation) = conversation else {
            warn!(conversation_id = %conversation_id, "conversation.activate_not_found");
            return Ok(None);
        };

        if conversation.is_pending() {
            let updated = self
                .repo
                .update_state(conversation_id, ConversationState::Active, None)
                .await?;
            if updated.is_some() {
                info!(conversation_id = %conversation_id, "conversation.activated");
            }
            Ok(updated)
        } else {
            Ok(Some(conversation))
        }
    }

    pub(crate) async fn close(
        &self,
        conversation_id: Uuid,
    ) -> Result<Option<Conversation>, AppError> {
        let conversation = self.repo.get_by_id(conversation_id).await?;
        let Some(conversation) = conversation else {
            warn!(conversation_id = %conversation_id, "conversation.close_not_found");
            return Ok(None);
        };

        if conversation.is_closed() {
            debug!(conversation_id = %conversation_id, "conversation.already_closed");
            return Ok(Some(conversation));
        }

        let updated = self
            .repo
            .update_state(conversation_id, ConversationState::Closed, None)
            .await?;
        if updated.is_some() {
            info!(conversation_id = %conversation_id, "conversation.closed");
        }
        Ok(updated)
    }

    pub(crate) async fn close_and_start_new(
        &self,
        conversation_id: Uuid,
        summary: Option<String>,
    ) -> Result<Conversation, AppError> {
        self.repo
            .update_state(conversation_id, ConversationState::Closed, summary)
            .await?;

        let new_conv = Conversation::new_pending();
        let saved = self.repo.save(&new_conv).await?;
        Ok(saved)
    }

    pub(crate) async fn get_last_closed_conversation(
        &self,
    ) -> Result<Option<Conversation>, AppError> {
        self.repo.get_last_closed().await
    }

    // ── Turn Management ─────────────────────────────────────────────────

    pub(crate) async fn add_turn(
        &self,
        conversation_id: Uuid,
        role: TurnRole,
        content: &str,
    ) -> Result<Option<ConversationTurn>, AppError> {
        let conversation = self.repo.get_by_id(conversation_id).await?;
        if conversation.is_none() {
            return Ok(None);
        }

        let turn = ConversationTurn::new(conversation_id, role, content.to_owned());
        let saved = self.repo.add_turn(&turn).await?;

        info!(
            conversation_id = %conversation_id,
            turn_id = %saved.id,
            role = %saved.role,
            content_length = content.len(),
            "conversation.turn_added"
        );
        Ok(Some(saved))
    }

    // ── Queries ─────────────────────────────────────────────────────────

    pub(crate) async fn get_pending_or_active(&self) -> Result<Option<Conversation>, AppError> {
        self.repo.get_pending_or_active().await
    }

    pub(crate) async fn get_recent_ai_messages(&self, limit: i64) -> Result<Vec<String>, AppError> {
        self.repo
            .get_recent_turns_by_role(TurnRole::Assistant, limit)
            .await
    }

    pub(crate) async fn get_conversation(
        &self,
        conversation_id: Uuid,
    ) -> Result<Option<Conversation>, AppError> {
        self.repo.get_by_id(conversation_id).await
    }

    pub(crate) async fn get_conversation_turns(
        &self,
        conversation_id: Uuid,
        limit: i64,
    ) -> Result<Vec<ConversationTurn>, AppError> {
        self.repo.get_turns(conversation_id, limit).await
    }

    pub(crate) async fn get_closed_since(
        &self,
        since: Option<DateTime<Utc>>,
    ) -> Result<Vec<Conversation>, AppError> {
        self.repo.find_closed_since(since).await
    }

    pub(crate) async fn get_previous_conversation_context(&self) -> Vec<(String, String)> {
        let Ok(Some(last_closed)) = self.get_last_closed_conversation().await else {
            return Vec::new();
        };

        let turns = match self.repo.get_turns(last_closed.id, 50).await {
            Ok(t) => t,
            Err(e) => {
                warn!(error = %e, "conversation_service.previous_context_load_failed");
                return Vec::new();
            }
        };

        turns
            .into_iter()
            .rev()
            .take(2)
            .map(|t| {
                let prefixed = format!("[From previous conversation] {}", t.content);
                (t.role.as_str().to_owned(), prefixed)
            })
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }
}

impl std::fmt::Debug for ConversationService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConversationService").finish()
    }
}
