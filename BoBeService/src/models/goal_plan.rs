use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::ids::{GoalId, GoalPlanId, GoalPlanStepId};
use super::types::{GoalPlanStatus, GoalPlanStepStatus};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub(crate) struct GoalPlan {
    pub(crate) id: GoalPlanId,
    pub(crate) goal_id: GoalId,
    pub(crate) summary: String,
    pub(crate) status: GoalPlanStatus,
    pub(crate) failure_count: i32,
    pub(crate) last_error: Option<String>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub(crate) struct GoalPlanStep {
    pub(crate) id: GoalPlanStepId,
    pub(crate) plan_id: GoalPlanId,
    pub(crate) step_order: i32,
    pub(crate) content: String,
    pub(crate) status: GoalPlanStepStatus,
    pub(crate) result: Option<String>,
    pub(crate) error: Option<String>,
    pub(crate) started_at: Option<DateTime<Utc>>,
    pub(crate) completed_at: Option<DateTime<Utc>>,
    pub(crate) created_at: DateTime<Utc>,
}
