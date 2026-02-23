use async_trait::async_trait;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;

use super::base::NativeTool;
use crate::error::AppError;
use crate::ports::embedding::EmbeddingProvider;
use crate::ports::repos::memory_repo::MemoryRepository;
use crate::ports::tools::{ToolCategory, ToolExecutionContext};

pub struct SearchContextTool {
    memory_repo: Arc<dyn MemoryRepository>,
    embedding_provider: Arc<dyn EmbeddingProvider>,
}

impl SearchContextTool {
    pub fn new(
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
impl NativeTool for SearchContextTool {
    fn name(&self) -> &str {
        "search_context"
    }

    fn description(&self) -> &str {
        "Semantic search over memories and raw context. Filters by category."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Natural language search query"
                },
                "categories": {
                    "type": "array",
                    "items": {
                        "type": "string",
                        "enum": ["preference", "pattern", "fact", "interest",
                                 "coding", "browsing", "communication",
                                 "documentation", "terminal", "general"]
                    },
                    "description": "Filter by memory categories"
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

        let categories: Option<Vec<&str>> = arguments
            .get("categories")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect());

        let embedding = self.embedding_provider.embed(query).await?;
        let results = self
            .memory_repo
            .find_similar(&embedding, limit, true, 0.6)
            .await?;

        let filtered: Vec<_> = if let Some(cats) = &categories {
            results
                .into_iter()
                .filter(|(m, _)| cats.contains(&m.category.as_str()))
                .collect()
        } else {
            results
        };

        if filtered.is_empty() {
            return Ok("No context found matching the query.".into());
        }

        let mut output = format!("Found {} context items:\n\n", filtered.len());
        for (i, (memory, score)) in filtered.iter().enumerate() {
            output.push_str(&format!(
                "{}. [{}] (score: {:.2}) {}\n   Category: {}\n\n",
                i + 1,
                memory.memory_type,
                score,
                memory.content,
                memory.category,
            ));
        }
        Ok(output)
    }
}
