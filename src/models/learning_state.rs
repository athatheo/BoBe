use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Tracks learning progress for resumability.
///
/// Single-row table — enforced by application logic.
/// Allows the learning loop to resume from where it left off after restarts.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct LearningState {
    pub id: Uuid,
    pub last_conversation_processed_at: Option<DateTime<Utc>>,
    pub last_context_processed_at: Option<DateTime<Utc>>,
    pub last_consolidation_at: Option<DateTime<Utc>>,
    pub last_pruning_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl LearningState {
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            last_conversation_processed_at: None,
            last_context_processed_at: None,
            last_consolidation_at: None,
            last_pruning_at: None,
            created_at: now,
            updated_at: now,
        }
    }
}

impl Default for LearningState {
    fn default() -> Self {
        Self::new()
    }
}
