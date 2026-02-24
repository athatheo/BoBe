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

pub struct ResumeGoalTool {
    goal_repo: Arc<dyn GoalRepository>,
}

impl ResumeGoalTool {
    pub fn new(goal_repo: Arc<dyn GoalRepository>) -> Self {
        Self { goal_repo }
    }
}

#[async_trait]
impl NativeTool for ResumeGoalTool {
    fn name(&self) -> &str {
        "resume_goal"
    }

    fn description(&self) -> &str {
        "Resume a paused goal"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "goal_id": {
                    "type": "string",
                    "description": "UUID of the goal to resume"
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

        if goal.status != GoalStatus::Paused {
            return Err(AppError::Validation(format!(
                "Goal {goal_id} is not paused (current status: {})",
                goal.status
            )));
        }

        self.goal_repo
            .update_status(goal_id, Some(GoalStatus::Active), None)
            .await?;

        Ok(format!(
            "Goal {} resumed.\nContent: {}",
            goal_id, goal.content,
        ))
    }
}
