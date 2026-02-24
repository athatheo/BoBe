use async_trait::async_trait;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use super::base::NativeTool;
use crate::models::types::GoalPlanStatus;
use crate::error::AppError;
use crate::db::GoalPlanRepository;
use crate::tools::{ToolCategory, ToolExecutionContext};

pub struct RejectPlanTool {
    goal_plan_repo: Arc<dyn GoalPlanRepository>,
}

impl RejectPlanTool {
    pub fn new(goal_plan_repo: Arc<dyn GoalPlanRepository>) -> Self {
        Self { goal_plan_repo }
    }
}

#[async_trait]
impl NativeTool for RejectPlanTool {
    fn name(&self) -> &str {
        "reject_plan"
    }

    fn description(&self) -> &str {
        "Reject a pending goal execution plan"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "plan_id": {
                    "type": "string",
                    "description": "UUID of the plan to reject"
                }
            },
            "required": ["plan_id"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Memory
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
            .update_plan_status(plan_id, GoalPlanStatus::Rejected, None)
            .await?;

        Ok(format!(
            "Plan {} rejected.\nSummary: {}",
            plan_id, plan.summary,
        ))
    }
}
