use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::types::{ConversationState, TurnRole};

/// A dialogue session with state machine.
///
/// State Machine:
///   PENDING (AI reached out, waiting for user)
///       ↓ (user responds)
///   ACTIVE (engaged conversation)
///       ↓ (timeout or explicit close)
///   CLOSED (ended, summary generated)
///
/// Invariant: Only ONE conversation can be ACTIVE at a time.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct Conversation {
    pub id: Uuid,
    pub state: ConversationState,
    pub closed_at: Option<DateTime<Utc>>,
    pub summary: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Conversation {
    pub fn new_pending() -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            state: ConversationState::Pending,
            closed_at: None,
            summary: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn new_active() -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            state: ConversationState::Active,
            closed_at: None,
            summary: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn is_pending(&self) -> bool {
        self.state == ConversationState::Pending
    }

    pub fn is_closed(&self) -> bool {
        self.state == ConversationState::Closed
    }

    /// Check if conversation is stale (no user activity within timeout).
    pub fn is_stale(&self, auto_close_minutes: i64, turns: &[ConversationTurn]) -> bool {
        let reference = self.last_user_message_at(turns).unwrap_or(self.created_at);
        let elapsed = Utc::now() - reference;
        elapsed >= chrono::Duration::minutes(auto_close_minutes)
    }

    pub fn last_user_message_at(&self, turns: &[ConversationTurn]) -> Option<DateTime<Utc>> {
        turns
            .iter()
            .rev()
            .find(|t| t.role == TurnRole::User)
            .map(|t| t.created_at)
    }
}

/// Individual message within a conversation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct ConversationTurn {
    pub id: Uuid,
    pub role: TurnRole,
    pub content: String,
    pub conversation_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ConversationTurn {
    pub fn new(conversation_id: Uuid, role: TurnRole, content: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            role,
            content,
            conversation_id,
            created_at: now,
            updated_at: now,
        }
    }
}
