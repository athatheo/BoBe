use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::types::{GoalPriority, GoalSource, GoalStatus};

/// User intention with semantic search capability.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct Goal {
    pub id: Uuid,
    pub content: String,
    pub priority: String,
    pub source: String,
    pub status: String,
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
            priority: priority.as_str().to_owned(),
            source: source.as_str().to_owned(),
            status: GoalStatus::Active.as_str().to_owned(),
            enabled: true,
            inference_reason: None,
            embedding: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn is_active(&self) -> bool {
        self.status == GoalStatus::Active.as_str()
    }

    pub fn is_completed(&self) -> bool {
        self.status == GoalStatus::Completed.as_str()
    }

    pub fn is_archived(&self) -> bool {
        self.status == GoalStatus::Archived.as_str()
    }

    pub fn complete(&mut self) {
        self.status = GoalStatus::Completed.as_str().to_owned();
        self.updated_at = Utc::now();
    }

    pub fn archive(&mut self) {
        self.status = GoalStatus::Archived.as_str().to_owned();
        self.updated_at = Utc::now();
    }
}
