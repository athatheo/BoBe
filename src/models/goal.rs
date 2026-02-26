use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::types::{GoalPriority, GoalSource, GoalStatus};

/// User intention with semantic search capability.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct Goal {
    pub id: Uuid,
    pub content: String,
    pub priority: GoalPriority,
    pub source: GoalSource,
    pub status: GoalStatus,
    pub enabled: bool,
    pub inference_reason: Option<String>,
    /// JSON-encoded embedding vector.
    pub embedding: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Goal {
    pub fn new(content: String, source: GoalSource, priority: GoalPriority) -> Self {
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

    pub fn is_active(&self) -> bool {
        self.status == GoalStatus::Active
    }

    pub fn is_completed(&self) -> bool {
        self.status == GoalStatus::Completed
    }

    pub fn is_archived(&self) -> bool {
        self.status == GoalStatus::Archived
    }
}
