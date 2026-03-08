//! ConversationService — manages conversation lifecycle and turn management.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::db::ConversationRepository;
use crate::error::AppError;
use crate::models::conversation::{Conversation, ConversationTurn};
use crate::models::ids::{ConversationId, ConversationTurnId};
use crate::models::types::{ConversationState, TurnRole};

#[derive(Debug, Clone)]
struct StreamingAssistantTurn {
    turn: ConversationTurn,
    persisted: bool,
}

pub(crate) struct ConversationService {
    repo: Arc<dyn ConversationRepository>,
    lifecycle_lock: Mutex<()>,
    streaming_assistant_turns: DashMap<ConversationId, StreamingAssistantTurn>,
}

impl ConversationService {
    pub(crate) fn new(repo: Arc<dyn ConversationRepository>) -> Self {
        Self {
            repo,
            lifecycle_lock: Mutex::new(()),
            streaming_assistant_turns: DashMap::new(),
        }
    }

    // ── Conversation Lifecycle ──────────────────────────────────────────

    pub(crate) async fn create_pending(&self, ai_message: &str) -> Result<Conversation, AppError> {
        self.append_assistant_turn_or_create_pending(ai_message)
            .await
    }

    pub(crate) async fn append_assistant_turn_or_create_pending(
        &self,
        ai_message: &str,
    ) -> Result<Conversation, AppError> {
        let _guard = self.lifecycle_lock.lock().await;
        if let Some(conversation) = self.repo.get_pending_or_active().await? {
            self.sync_streaming_assistant_turn_locked(conversation.id)
                .await?;
            let turn =
                ConversationTurn::new(conversation.id, TurnRole::Assistant, ai_message.to_owned());
            self.repo.add_turn(&turn).await?;
            info!(
                conversation_id = %conversation.id,
                state = %conversation.state,
                "conversation.assistant_turn_appended"
            );
            return Ok(conversation);
        }

        self.create_pending_unlocked(ai_message).await
    }

    pub(crate) async fn append_user_turn_or_create_active(
        &self,
        user_message: &str,
    ) -> Result<Conversation, AppError> {
        let _guard = self.lifecycle_lock.lock().await;
        if let Some(conversation) = self.repo.get_pending_or_active().await? {
            self.sync_streaming_assistant_turn_locked(conversation.id)
                .await?;
            let conversation = if conversation.is_pending() {
                self.repo
                    .update_state(conversation.id, ConversationState::Active, None)
                    .await?
                    .ok_or_else(|| {
                        AppError::NotFound(format!(
                            "Conversation {} not found while activating pending conversation",
                            conversation.id
                        ))
                    })?
            } else {
                conversation
            };

            let turn =
                ConversationTurn::new(conversation.id, TurnRole::User, user_message.to_owned());
            self.repo.add_turn(&turn).await?;
            info!(
                conversation_id = %conversation.id,
                state = %conversation.state,
                "conversation.user_turn_appended"
            );
            return Ok(conversation);
        }

        self.create_active_unlocked(user_message).await
    }

    pub(crate) async fn begin_proactive_stream(
        &self,
        preferred_conversation: Option<&Conversation>,
    ) -> Result<Conversation, AppError> {
        let _guard = self.lifecycle_lock.lock().await;

        let conversation = if let Some(preferred) = preferred_conversation {
            match self.repo.get_by_id(preferred.id).await? {
                Some(current) if !current.is_closed() => current,
                _ => self.open_or_create_pending_locked().await?,
            }
        } else {
            self.open_or_create_pending_locked().await?
        };

        let turn = ConversationTurn::new_with_id(
            ConversationTurnId::new(),
            conversation.id,
            TurnRole::Assistant,
            String::new(),
        );
        let replaced = self.streaming_assistant_turns.insert(
            conversation.id,
            StreamingAssistantTurn {
                turn,
                persisted: false,
            },
        );
        if replaced.is_some() {
            warn!(
                conversation_id = %conversation.id,
                "conversation.streaming_turn_replaced"
            );
        }

        Ok(conversation)
    }

    pub(crate) fn push_proactive_stream_delta(&self, conversation_id: ConversationId, delta: &str) {
        if delta.is_empty() {
            return;
        }

        if let Some(mut draft) = self.streaming_assistant_turns.get_mut(&conversation_id) {
            draft.turn.append_content(delta);
        }
    }

    pub(crate) async fn finalize_proactive_stream(
        &self,
        conversation_id: ConversationId,
        final_content: &str,
    ) -> Result<Option<ConversationTurn>, AppError> {
        let _guard = self.lifecycle_lock.lock().await;
        let Some((_, mut draft)) = self.streaming_assistant_turns.remove(&conversation_id) else {
            return Ok(None);
        };

        draft.turn.replace_content(final_content.to_owned());
        if draft.turn.content.is_empty() {
            return Ok(None);
        }

        if draft.persisted {
            self.repo
                .update_turn_content(draft.turn.id, &draft.turn.content)
                .await
        } else {
            self.repo.add_turn(&draft.turn).await.map(Some)
        }
    }

    pub(crate) fn discard_proactive_stream(&self, conversation_id: ConversationId) {
        self.streaming_assistant_turns.remove(&conversation_id);
    }

    async fn create_pending_unlocked(&self, ai_message: &str) -> Result<Conversation, AppError> {
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

    async fn open_or_create_pending_locked(&self) -> Result<Conversation, AppError> {
        if let Some(conversation) = self.repo.get_pending_or_active().await? {
            return Ok(conversation);
        }

        let conversation = Conversation::new_pending();
        self.repo.save(&conversation).await
    }

    async fn sync_streaming_assistant_turn_locked(
        &self,
        conversation_id: ConversationId,
    ) -> Result<(), AppError> {
        let Some(snapshot) = self
            .streaming_assistant_turns
            .get(&conversation_id)
            .map(|draft| (draft.turn.clone(), draft.persisted))
        else {
            return Ok(());
        };

        let (turn, persisted) = snapshot;
        if turn.content.is_empty() {
            return Ok(());
        }

        if persisted {
            self.repo
                .update_turn_content(turn.id, &turn.content)
                .await?;
        } else {
            self.repo.add_turn(&turn).await?;
            if let Some(mut draft) = self.streaming_assistant_turns.get_mut(&conversation_id)
                && draft.turn.id == turn.id
            {
                draft.persisted = true;
            }
        }

        Ok(())
    }

    async fn create_active_unlocked(&self, user_message: &str) -> Result<Conversation, AppError> {
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

    pub(crate) async fn close_and_start_new(
        &self,
        conversation_id: ConversationId,
        summary: Option<String>,
    ) -> Result<Conversation, AppError> {
        let _guard = self.lifecycle_lock.lock().await;
        self.streaming_assistant_turns.remove(&conversation_id);
        self.repo
            .update_state(conversation_id, ConversationState::Closed, summary)
            .await?;

        let new_conv = Conversation::new_pending();
        let saved = self.repo.save(&new_conv).await?;
        Ok(saved)
    }

    pub(crate) async fn close_if_stale(
        &self,
        conversation_id: ConversationId,
        auto_close_minutes: i64,
        turn_limit: i64,
    ) -> Result<Option<usize>, AppError> {
        let _guard = self.lifecycle_lock.lock().await;

        if self
            .streaming_assistant_turns
            .contains_key(&conversation_id)
        {
            return Ok(None);
        }

        let conversation = self.repo.get_by_id(conversation_id).await?;
        let Some(conversation) = conversation else {
            return Ok(None);
        };

        let turns = self.repo.get_turns(conversation_id, turn_limit).await?;
        if !conversation.is_stale(auto_close_minutes, &turns) {
            return Ok(None);
        }

        let updated = self
            .repo
            .update_state(conversation_id, ConversationState::Closed, None)
            .await?;
        if updated.is_some() {
            info!(conversation_id = %conversation_id, "conversation.closed");
            return Ok(Some(turns.len()));
        }

        Ok(None)
    }

    pub(crate) async fn get_last_closed_conversation(
        &self,
    ) -> Result<Option<Conversation>, AppError> {
        self.repo.get_last_closed().await
    }

    // ── Turn Management ─────────────────────────────────────────────────

    pub(crate) async fn add_turn(
        &self,
        conversation_id: ConversationId,
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
        conversation_id: ConversationId,
    ) -> Result<Option<Conversation>, AppError> {
        self.repo.get_by_id(conversation_id).await
    }

    pub(crate) async fn get_conversation_turns(
        &self,
        conversation_id: ConversationId,
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
