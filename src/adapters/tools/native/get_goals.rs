use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

use super::base::NativeTool;
use crate::error::AppError;
use crate::ports::repos::goal_repo::GoalRepository;
use crate::ports::tools::{ToolCategory, ToolExecutionContext};

pub struct GetGoalsTool {
    goal_repo: Arc<dyn GoalRepository>,
}

impl GetGoalsTool {
    pub fn new(goal_repo: Arc<dyn GoalRepository>) -> Self {
        Self { goal_repo }
    }
}

#[async_trait]
impl NativeTool for GetGoalsTool {
    fn name(&self) -> &str {
        "get_goals"
    }

    fn description(&self) -> &str {
        "Get all active goals ordered by priority."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Memory
    }

    async fn execute(
        &self,
        _arguments: HashMap<String, Value>,
        _context: Option<&ToolExecutionContext>,
    ) -> Result<String, AppError> {
        let goals = self.goal_repo.find_active(true).await?;

        if goals.is_empty() {
            return Ok("No active goals found.".into());
        }

        let mut output = format!("{} active goals:\n\n", goals.len());
        for (i, goal) in goals.iter().enumerate() {
            let priority_label = match goal.priority.as_str() {
                "high" => "🔴 HIGH",
                "medium" => "🟡 MEDIUM",
                "low" => "🟢 LOW",
                other => other,
            };
            output.push_str(&format!(
                "{}. [{}] {} {}\n   Source: {} | Created: {}\n\n",
                i + 1,
                goal.id,
                priority_label,
                goal.content,
                goal.source,
                goal.created_at.format("%Y-%m-%d"),
            ));
        }
        Ok(output)
    }
}
