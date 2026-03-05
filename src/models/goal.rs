use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::types::{GoalPriority, GoalSource, GoalStatus};

/// User intention with semantic search capability.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub(crate) struct Goal {
    pub(crate) id: Uuid,
    pub(crate) content: String,
    pub(crate) priority: GoalPriority,
    pub(crate) source: GoalSource,
    pub(crate) status: GoalStatus,
    pub(crate) enabled: bool,
    pub(crate) inference_reason: Option<String>,
    /// JSON-encoded embedding vector.
    pub(crate) embedding: Option<String>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

impl Goal {
    pub(crate) fn new(content: String, source: GoalSource, priority: GoalPriority) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            content,
            priority,
            source,
            status: GoalStatus::Active,
            enabled: true,
            inference_reason: None,
            embedding: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub(crate) fn is_active(&self) -> bool {
        self.status == GoalStatus::Active
    }

    pub(crate) fn is_completed(&self) -> bool {
        self.status == GoalStatus::Completed
    }

    pub(crate) fn is_archived(&self) -> bool {
        self.status == GoalStatus::Archived
    }
}
