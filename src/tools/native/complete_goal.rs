use async_trait::async_trait;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use super::base::NativeTool;
use crate::models::types::GoalStatus;
use crate::error::AppError;
use crate::db::GoalRepository;
use crate::tools::{ToolCategory, ToolExecutionContext};

pub struct CompleteGoalTool {
    goal_repo: Arc<dyn GoalRepository>,
}

impl CompleteGoalTool {
    pub fn new(goal_repo: Arc<dyn GoalRepository>) -> Self {
        Self { goal_repo }
    }
}

#[async_trait]
impl NativeTool for CompleteGoalTool {
    fn name(&self) -> &str {
        "complete_goal"
    }

    fn description(&self) -> &str {
        "Mark a goal as completed."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "goal_id": {
                    "type": "string",
                    "description": "UUID of the goal to complete"
                }
            },
            "required": ["goal_id"]
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
        let goal_id_str = arguments
            .get("goal_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Validation("'goal_id' is required".into()))?;

        let goal_id = Uuid::parse_str(goal_id_str)
            .map_err(|_| AppError::Validation(format!("Invalid UUID: {goal_id_str}")))?;

        let goal = self
            .goal_repo
            .get_by_id(goal_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Goal {goal_id} not found")))?;

        if goal.is_completed() {
            return Ok(format!("Goal {} is already completed.", goal_id));
        }

        self.goal_repo
            .update_status(goal_id, Some(GoalStatus::Completed), None)
            .await?;

        Ok(format!(
            "Goal {} marked as completed.\nContent: {}",
            goal_id, goal.content,
        ))
    }
}
