use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

use super::base::NativeTool;
use crate::error::AppError;
use crate::ports::embedding::EmbeddingProvider;
use crate::ports::repos::goal_repo::GoalRepository;
use crate::ports::tools::{ToolCategory, ToolExecutionContext};

pub struct SearchGoalTool {
    goal_repo: Arc<dyn GoalRepository>,
    embedding_provider: Arc<dyn EmbeddingProvider>,
}

impl SearchGoalTool {
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
impl NativeTool for SearchGoalTool {
    fn name(&self) -> &str {
        "search_goal"
    }

    fn description(&self) -> &str {
        "Semantic search over user goals. Find goals by meaning, not exact text."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Natural language goal search query"
                },
                "include_completed": {
                    "type": "boolean",
                    "description": "Include completed goals (default: false)",
                    "default": false
                },
                "priority_filter": {
                    "type": "string",
                    "enum": ["high", "medium", "low"],
                    "description": "Filter by priority level"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum results (default: 5, max: 20)",
                    "default": 5
                }
            },
            "required": ["query"]
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
        let query = arguments
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Validation("'query' parameter is required".into()))?;

        let limit = arguments
            .get("limit")
            .and_then(|v| v.as_i64())
            .unwrap_or(5)
            .clamp(1, 20);

        let include_completed = arguments
            .get("include_completed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let priority_filter = arguments
            .get("priority_filter")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned());

        let embedding = self.embedding_provider.embed(query).await?;
        let results = self
            .goal_repo
            .find_similar(&embedding, limit, true)
            .await?;

        let filtered: Vec<_> = results
            .into_iter()
            .filter(|(g, _)| include_completed || g.is_active())
            .filter(|(g, _)| {
                priority_filter
                    .as_ref()
                    .is_none_or(|p| g.priority == *p)
            })
            .collect();

        if filtered.is_empty() {
            return Ok("No goals found matching the query.".into());
        }

        let mut output = format!("Found {} goals:\n\n", filtered.len());
        for (i, (goal, score)) in filtered.iter().enumerate() {
            output.push_str(&format!(
                "{}. (score: {:.2}) [{}] {}\n   Priority: {} | Status: {} | Source: {}\n\n",
                i + 1,
                score,
                goal.id,
                goal.content,
                goal.priority,
                goal.status,
                goal.source,
            ));
        }
        Ok(output)
    }
}
