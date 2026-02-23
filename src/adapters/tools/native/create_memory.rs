use async_trait::async_trait;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;

use super::base::NativeTool;
use crate::domain::memory::Memory;
use crate::domain::types::{MemorySource, MemoryType};
use crate::error::AppError;
use crate::ports::embedding::EmbeddingProvider;
use crate::ports::repos::memory_repo::MemoryRepository;
use crate::ports::tools::{ToolCategory, ToolExecutionContext};

const VALID_CATEGORIES: &[&str] = &["preference", "pattern", "fact", "interest"];

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
                    "description": "The memory to store (5-1000 chars). Format as a clear statement.",
                    "minLength": 5,
                    "maxLength": 1000
                },
                "category": {
                    "type": "string",
                    "enum": ["preference", "pattern", "fact", "interest"],
                    "description": "Memory category"
                }
            },
            "required": ["content", "category"]
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

        if content.len() < 5 || content.len() > 1000 {
            return Err(AppError::Validation(
                "Content must be between 5 and 1000 characters".into(),
            ));
        }

        let cat = arguments
            .get("category")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Validation("'category' is required".into()))?;

        if !VALID_CATEGORIES.contains(&cat) {
            return Err(AppError::Validation(format!(
                "Invalid category '{}'. Must be one of: {}",
                cat,
                VALID_CATEGORIES.join(", ")
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
