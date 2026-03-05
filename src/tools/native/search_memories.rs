use async_trait::async_trait;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fmt::Write;
use std::sync::Arc;

use super::base::NativeTool;
use crate::constants::{TOOL_LIMIT_MAX, TOOL_LIMIT_MIN};
use crate::db::MemoryRepository;
use crate::error::AppError;
use crate::llm::EmbeddingProvider;
use crate::tools::ToolExecutionContext;

pub(crate) struct SearchMemoriesTool {
    memory_repo: Arc<dyn MemoryRepository>,
    embedding_provider: Arc<dyn EmbeddingProvider>,
}

impl SearchMemoriesTool {
    pub(crate) fn new(
        memory_repo: Arc<dyn MemoryRepository>,
        embedding_provider: Arc<dyn EmbeddingProvider>,
    ) -> Self {
        Self {
            memory_repo,
            embedding_provider,
        }
    }
}

#[async_trait]
impl NativeTool for SearchMemoriesTool {
    fn name(&self) -> &str {
        "search_memories"
    }

    fn description(&self) -> &str {
        "Semantic search over stored user memories. Use to find relevant preferences, facts, patterns, and interests."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query describing what to find"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of memories to return (default: 5, max: 20)",
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

        let embedding = self.embedding_provider.embed(query).await?;
        let results = self
            .memory_repo
            .find_similar(&embedding, limit, true, 0.6)
            .await?;

        if results.is_empty() {
            return Ok("No memories found matching the query.".into());
        }

        let mut output = format!("Found {} memories:\n\n", results.len());
        for (i, (memory, score)) in results.iter().enumerate() {
            let _ = write!(
                output,
                "{}. [{}] (score: {:.2}) {}\n   Category: {} | Created: {}\n\n",
                i + 1,
                memory.memory_type,
                score,
                memory.content,
                memory.category,
                memory.created_at.format("%Y-%m-%d %H:%M"),
            );
        }
        Ok(output)
    }
}
