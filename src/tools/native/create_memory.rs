use async_trait::async_trait;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;

use super::base::NativeTool;
use crate::constants::{
    MEMORY_CONTENT_MAX_LENGTH, MEMORY_CONTENT_MIN_LENGTH, VALID_MEMORY_CATEGORIES,
};
use crate::db::MemoryRepository;
use crate::error::AppError;
use crate::llm::EmbeddingProvider;
use crate::models::memory::Memory;
use crate::models::types::{MemorySource, MemoryType};
use crate::tools::ToolExecutionContext;

pub struct CreateMemoryTool {
    memory_repo: Arc<dyn MemoryRepository>,
    embedding_provider: Arc<dyn EmbeddingProvider>,
}

impl CreateMemoryTool {
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
impl NativeTool for CreateMemoryTool {
    fn name(&self) -> &str {
        "create_memory"
    }

    fn description(&self) -> &str {
        "Explicitly store a new memory when the user requests it. Use for 'remember this' type requests."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": format!(
                        "The memory to store ({}-{} chars). Format as a clear statement.",
                        MEMORY_CONTENT_MIN_LENGTH,
                        MEMORY_CONTENT_MAX_LENGTH
                    ),
                    "minLength": MEMORY_CONTENT_MIN_LENGTH,
                    "maxLength": MEMORY_CONTENT_MAX_LENGTH
                },
                "category": {
                    "type": "string",
                    "enum": VALID_MEMORY_CATEGORIES,
                    "description": "Memory category"
                }
            },
            "required": ["content", "category"]
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

        if content.len() < MEMORY_CONTENT_MIN_LENGTH || content.len() > MEMORY_CONTENT_MAX_LENGTH {
            return Err(AppError::Validation(format!(
                "Content must be between {MEMORY_CONTENT_MIN_LENGTH} and {MEMORY_CONTENT_MAX_LENGTH} characters"
            )));
        }

        let cat = arguments
            .get("category")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Validation("'category' is required".into()))?;

        if !VALID_MEMORY_CATEGORIES.contains(&cat) {
            return Err(AppError::Validation(format!(
                "Invalid category '{}'. Must be one of: {}",
                cat,
                VALID_MEMORY_CATEGORIES.join(", ")
            )));
        }

        let embedding = self.embedding_provider.embed(content).await?;
        let mut memory = Memory::new(
            content.to_owned(),
            MemoryType::Explicit,
            MemorySource::Conversation,
            cat.to_owned(),
        );
        memory.embedding = Some(serde_json::to_string(&embedding)?);

        let saved = self.memory_repo.save(&memory).await?;
        Ok(format!(
            "Memory stored successfully.\nID: {}\nCategory: {}\nContent: {}",
            saved.id, cat, content
        ))
    }
}
