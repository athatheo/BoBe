use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::types::{GoalPlanStatus, GoalPlanStepStatus};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct GoalPlan {
    pub id: Uuid,
    pub goal_id: Uuid,
    pub summary: String,
    pub status: GoalPlanStatus,
    pub failure_count: i32,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct GoalPlanStep {
    pub id: Uuid,
    pub plan_id: Uuid,
    pub step_order: i32,
    pub content: String,
    pub status: GoalPlanStepStatus,
    pub result: Option<String>,
    pub error: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}
