use async_trait::async_trait;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use super::base::NativeTool;
use crate::db::GoalPlanRepository;
use crate::error::AppError;
use crate::models::types::GoalPlanStatus;
use crate::tools::ToolExecutionContext;

pub struct ApprovePlanTool {
    goal_plan_repo: Arc<dyn GoalPlanRepository>,
}

impl ApprovePlanTool {
    pub fn new(goal_plan_repo: Arc<dyn GoalPlanRepository>) -> Self {
        Self { goal_plan_repo }
    }
}

#[async_trait]
impl NativeTool for ApprovePlanTool {
    fn name(&self) -> &str {
        "approve_plan"
    }

    fn description(&self) -> &str {
        "Approve a pending goal execution plan"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "plan_id": {
                    "type": "string",
                    "description": "UUID of the plan to approve"
                }
            },
            "required": ["plan_id"]
        })
    }

    async fn execute(
        &self,
        arguments: HashMap<String, Value>,
        _context: Option<&ToolExecutionContext>,
    ) -> Result<String, AppError> {
        let plan_id_str = arguments
            .get("plan_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Validation("'plan_id' is required".into()))?;

        let plan_id = Uuid::parse_str(plan_id_str)
            .map_err(|_| AppError::Validation(format!("Invalid UUID: {plan_id_str}")))?;

        let plan = self
            .goal_plan_repo
            .get_plan(plan_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Plan {plan_id} not found")))?;

        if plan.status != GoalPlanStatus::PendingApproval {
            return Err(AppError::Validation(format!(
                "Plan {plan_id} is not pending approval (current status: {})",
                plan.status
            )));
        }

        self.goal_plan_repo
            .update_plan_status(plan_id, GoalPlanStatus::Approved, None)
            .await?;

        Ok(format!(
            "Plan {} approved and queued for execution.\nSummary: {}",
            plan_id, plan.summary,
        ))
    }
}
