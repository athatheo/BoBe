use chrono::{DateTime, Utc};

use super::ids::LearningStateId;

/// Single-row table tracking learning progress for resumability across restarts.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub(crate) struct LearningState {
    pub(crate) id: LearningStateId,
    pub(crate) last_conversation_processed_at: Option<DateTime<Utc>>,
    pub(crate) last_context_processed_at: Option<DateTime<Utc>>,
    pub(crate) last_consolidation_at: Option<DateTime<Utc>>,
    pub(crate) last_pruning_at: Option<DateTime<Utc>>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

impl LearningState {
    pub(crate) fn new() -> Self {
        let now = Utc::now();
        Self {
            id: LearningStateId::new(),
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
