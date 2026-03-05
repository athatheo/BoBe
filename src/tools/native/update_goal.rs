use async_trait::async_trait;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use super::base::NativeTool;
use crate::db::GoalRepository;
use crate::error::AppError;
use crate::models::types::GoalStatus;
use crate::tools::ToolExecutionContext;

pub(crate) struct UpdateGoalTool {
    goal_repo: Arc<dyn GoalRepository>,
}

impl UpdateGoalTool {
    pub(crate) fn new(goal_repo: Arc<dyn GoalRepository>) -> Self {
        Self { goal_repo }
    }
}

#[async_trait]
impl NativeTool for UpdateGoalTool {
    fn name(&self) -> &str {
        "update_goal"
    }

    fn description(&self) -> &str {
        "Update a goal's status. Use goal_id from search_goal or get_goals results."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "goal_id": {
                    "type": "string",
                    "description": "UUID of the goal to update"
                },
                "status": {
                    "type": "string",
                    "enum": ["active", "completed", "archived"],
                    "description": "New status for the goal"
                }
            },
            "required": ["goal_id", "status"]
        })
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

        let status_str = arguments
            .get("status")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Validation("'status' is required".into()))?;

        let status = match status_str {
            "active" => GoalStatus::Active,
            "completed" => GoalStatus::Completed,
            "archived" => GoalStatus::Archived,
            other => {
                return Err(AppError::Validation(format!(
                    "Invalid status '{other}'. Must be active, completed, or archived"
                )));
            }
        };

        let old = self
            .goal_repo
            .get_by_id(goal_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Goal {goal_id} not found")))?;

        let updated = self
            .goal_repo
            .update_status(goal_id, Some(status), None)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Goal {goal_id} not found")))?;

        Ok(format!(
            "Goal {} updated.\nStatus: {} → {}\nContent: {}",
            updated.id, old.status, updated.status, updated.content,
        ))
    }
}
