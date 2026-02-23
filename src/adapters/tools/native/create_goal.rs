use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

use super::base::NativeTool;
use crate::domain::goal::Goal;
use crate::domain::types::{GoalPriority, GoalSource};
use crate::error::AppError;
use crate::ports::embedding::EmbeddingProvider;
use crate::ports::repos::goal_repo::GoalRepository;
use crate::ports::tools::{ToolCategory, ToolExecutionContext};

pub struct CreateGoalTool {
    goal_repo: Arc<dyn GoalRepository>,
    embedding_provider: Arc<dyn EmbeddingProvider>,
}

impl CreateGoalTool {
    pub fn new(
        goal_repo: Arc<dyn GoalRepository>,
        embedding_provider: Arc<dyn EmbeddingProvider>,
    ) -> Self {
        Self {
            goal_repo,
            embedding_provider,
        }
    }
}

#[async_trait]
impl NativeTool for CreateGoalTool {
    fn name(&self) -> &str {
        "create_goal"
    }

    fn description(&self) -> &str {
        "Create a new user goal. Checks for duplicates before creating."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "Goal description (5-500 chars)",
                    "minLength": 5,
                    "maxLength": 500
                },
                "priority": {
                    "type": "string",
                    "enum": ["high", "medium", "low"],
                    "description": "Goal priority (default: medium)",
                    "default": "medium"
                }
            },
            "required": ["content"]
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
        let content = arguments
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Validation("'content' is required".into()))?;

        if content.len() < 5 || content.len() > 500 {
            return Err(AppError::Validation(
                "Content must be between 5 and 500 characters".into(),
            ));
        }

        let priority_str = arguments
            .get("priority")
            .and_then(|v| v.as_str())
            .unwrap_or("medium");

        let priority = match priority_str {
            "high" => GoalPriority::High,
            "medium" => GoalPriority::Medium,
            "low" => GoalPriority::Low,
            other => {
                return Err(AppError::Validation(format!(
                    "Invalid priority '{other}'. Must be high, medium, or low"
                )));
            }
        };

        // Check for duplicates via semantic similarity
        let embedding = self.embedding_provider.embed(content).await?;
        let similar = self.goal_repo.find_similar(&embedding, 1, true).await?;
        if let Some((existing, score)) = similar.first() {
            if *score > 0.85 {
                return Ok(format!(
                    "A similar goal already exists (similarity: {:.0}%):\n[{}] {}\nNo new goal created.",
                    score * 100.0,
                    existing.id,
                    existing.content,
                ));
            }
        }

        let mut goal = Goal::new(content.to_owned(), GoalSource::User, priority);
        goal.embedding = Some(serde_json::to_string(&embedding)?);

        let saved = self.goal_repo.save(&goal).await?;
        Ok(format!(
            "Goal created successfully.\nID: {}\nPriority: {}\nContent: {}",
            saved.id, priority_str, content
        ))
    }
}
