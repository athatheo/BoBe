use chrono::{DateTime, Utc};

use super::ids::{ConversationId, ConversationTurnId};
use super::types::{ConversationState, TurnRole};

/// Invariant: only one conversation open (`PENDING` or `ACTIVE`) at a time.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub(crate) struct Conversation {
    pub(crate) id: ConversationId,
    pub(crate) state: ConversationState,
    pub(crate) closed_at: Option<DateTime<Utc>>,
    pub(crate) summary: Option<String>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

impl Conversation {
    pub(crate) fn new_pending() -> Self {
        let now = Utc::now();
        Self {
            id: ConversationId::new(),
            state: ConversationState::Pending,
            closed_at: None,
            summary: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub(crate) fn new_active() -> Self {
        let now = Utc::now();
        Self {
            id: ConversationId::new(),
            state: ConversationState::Active,
            closed_at: None,
            summary: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub(crate) fn is_pending(&self) -> bool {
        self.state == ConversationState::Pending
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.state == ConversationState::Closed
    }

    /// Lighter variant: takes the last user-turn timestamp directly so
    /// callers can sidestep loading every turn row just to find the most
    /// recent user message.
    pub(crate) fn is_stale_since(
        &self,
        auto_close_minutes: i64,
        last_user_at: Option<DateTime<Utc>>,
    ) -> bool {
        let reference = last_user_at.unwrap_or(self.created_at);
        let elapsed = Utc::now() - reference;
        elapsed >= chrono::Duration::minutes(auto_close_minutes)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub(crate) struct ConversationTurn {
    pub(crate) id: ConversationTurnId,
    pub(crate) role: TurnRole,
    pub(crate) content: String,
    pub(crate) is_complete: bool,
    pub(crate) conversation_id: ConversationId,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

impl ConversationTurn {
    pub(crate) fn new(conversation_id: ConversationId, role: TurnRole, content: String) -> Self {
        Self::new_with_id(ConversationTurnId::new(), conversation_id, role, content)
    }

    pub(crate) fn new_with_id(
        id: ConversationTurnId,
        conversation_id: ConversationId,
        role: TurnRole,
        content: String,
    ) -> Self {
        Self::new_with_completion(id, conversation_id, role, content, true)
    }

    pub(crate) fn new_with_completion(
        id: ConversationTurnId,
        conversation_id: ConversationId,
        role: TurnRole,
        content: String,
        is_complete: bool,
    ) -> Self {
        let now = Utc::now();
        Self {
            id,
            role,
            content,
            is_complete,
            conversation_id,
            created_at: now,
            updated_at: now,
        }
    }

    pub(crate) fn append_content(&mut self, delta: &str) {
        self.content.push_str(delta);
        self.updated_at = Utc::now();
    }

    pub(crate) fn replace_content(&mut self, content: String) {
        self.content = content;
        self.updated_at = Utc::now();
    }

    pub(crate) fn set_complete(&mut self, is_complete: bool) {
        self.is_complete = is_complete;
        self.updated_at = Utc::now();
    }
}
