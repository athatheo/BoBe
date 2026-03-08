use chrono::{DateTime, Utc};

use super::ids::{ConversationId, ConversationTurnId};
use super::types::{ConversationState, TurnRole};

/// Dialogue session. States: PENDING → ACTIVE → CLOSED.
///
/// Invariant: only one conversation should be open (`PENDING` or `ACTIVE`) at a time.
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

    /// True if no user activity within `auto_close_minutes`.
    pub(crate) fn is_stale(&self, auto_close_minutes: i64, turns: &[ConversationTurn]) -> bool {
        let reference = self.last_user_message_at(turns).unwrap_or(self.created_at);
        let elapsed = Utc::now() - reference;
        elapsed >= chrono::Duration::minutes(auto_close_minutes)
    }

    pub(crate) fn last_user_message_at(&self, turns: &[ConversationTurn]) -> Option<DateTime<Utc>> {
        turns
            .iter()
            .rev()
            .find(|t| t.role == TurnRole::User)
            .map(|t| t.created_at)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub(crate) struct ConversationTurn {
    pub(crate) id: ConversationTurnId,
    pub(crate) role: TurnRole,
    pub(crate) content: String,
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
        let now = Utc::now();
        Self {
            id,
            role,
            content,
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
}
