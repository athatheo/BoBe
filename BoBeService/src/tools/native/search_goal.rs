use async_trait::async_trait;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fmt::Write;
use std::sync::Arc;

use super::base::NativeTool;
use crate::constants::{TOOL_LIMIT_MAX, TOOL_LIMIT_MIN};
use crate::db::GoalRepository;
use crate::error::AppError;
use crate::llm::EmbeddingProvider;
use crate::tools::ToolExecutionContext;

pub(crate) struct SearchGoalTool {
    goal_repo: Arc<dyn GoalRepository>,
    embedding_provider: Arc<dyn EmbeddingProvider>,
}

impl SearchGoalTool {
    pub(crate) fn new(
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
            .and_then(Value::as_i64)
            .unwrap_or(5)
            .clamp(TOOL_LIMIT_MIN, TOOL_LIMIT_MAX);

        let include_completed = arguments
            .get("include_completed")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let priority_filter = arguments
            .get("priority_filter")
            .and_then(Value::as_str)
            .map(str::to_owned);

        let embedding = self.embedding_provider.embed(query).await?;
        let results = self.goal_repo.find_similar(&embedding, limit, true).await?;

        let filtered: Vec<_> = results
            .into_iter()
            .filter(|(g, _)| include_completed || g.is_active())
            .filter(|(g, _)| {
                priority_filter
                    .as_ref()
                    .is_none_or(|p| g.priority.as_str() == p.as_str())
            })
            .collect();

        if filtered.is_empty() {
            return Ok("No goals found matching the query.".into());
        }

        let mut output = format!("Found {} goals:\n\n", filtered.len());
        for (i, (goal, score)) in filtered.iter().enumerate() {
            let _ = write!(
                output,
                "{}. (score: {:.2}) [{}] {}\n   Priority: {} | Status: {} | Source: {}\n\n",
                i + 1,
                score,
                goal.id,
                goal.content,
                goal.priority,
                goal.status,
                goal.source,
            );
        }
        Ok(output)
    }
}
