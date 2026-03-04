use async_trait::async_trait;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;

use super::base::NativeTool;
use crate::constants::{GOAL_CONTENT_MAX_LENGTH, GOAL_CONTENT_MIN_LENGTH};
use crate::db::GoalRepository;
use crate::error::AppError;
use crate::llm::EmbeddingProvider;
use crate::models::goal::Goal;
use crate::models::types::{GoalPriority, GoalSource};
use crate::tools::ToolExecutionContext;

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
                    "description": format!(
                        "Goal description ({}-{} chars)",
                        GOAL_CONTENT_MIN_LENGTH,
                        GOAL_CONTENT_MAX_LENGTH
                    ),
                    "minLength": GOAL_CONTENT_MIN_LENGTH,
                    "maxLength": GOAL_CONTENT_MAX_LENGTH
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

    async fn execute(
        &self,
        arguments: HashMap<String, Value>,
        _context: Option<&ToolExecutionContext>,
    ) -> Result<String, AppError> {
        let content = arguments
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Validation("'content' is required".into()))?;

        if content.len() < GOAL_CONTENT_MIN_LENGTH || content.len() > GOAL_CONTENT_MAX_LENGTH {
            return Err(AppError::Validation(format!(
                "Content must be between {GOAL_CONTENT_MIN_LENGTH} and {GOAL_CONTENT_MAX_LENGTH} characters"
            )));
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
        if let Some((existing, score)) = similar.first()
            && *score > 0.85
        {
            return Ok(format!(
                "A similar goal already exists (similarity: {:.0}%):\n[{}] {}\nNo new goal created.",
                score * 100.0,
                existing.id,
                existing.content,
            ));
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
