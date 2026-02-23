use async_trait::async_trait;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use super::base::NativeTool;
use crate::error::AppError;
use crate::db::MemoryRepository;
use crate::tools::{ToolCategory, ToolExecutionContext};

const VALID_CATEGORIES: &[&str] = &["preference", "pattern", "fact", "interest"];

pub struct UpdateMemoryTool {
    memory_repo: Arc<dyn MemoryRepository>,
}

impl UpdateMemoryTool {
    pub fn new(memory_repo: Arc<dyn MemoryRepository>) -> Self {
        Self { memory_repo }
    }
}

#[async_trait]
impl NativeTool for UpdateMemoryTool {
    fn name(&self) -> &str {
        "update_memory"
    }

    fn description(&self) -> &str {
        "Update a memory's category or enabled state. Use memory_id from search_context results."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "memory_id": {
                    "type": "string",
                    "description": "UUID of the memory to update"
                },
                "enabled": {
                    "type": "boolean",
                    "description": "Set to false to disable memory from search results"
                },
                "category": {
                    "type": "string",
                    "enum": ["preference", "pattern", "fact", "interest"],
                    "description": "New category for the memory"
                }
            },
            "required": ["memory_id"]
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
        let memory_id_str = arguments
            .get("memory_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Validation("'memory_id' is required".into()))?;

        let memory_id = Uuid::parse_str(memory_id_str)
            .map_err(|_| AppError::Validation(format!("Invalid UUID: {memory_id_str}")))?;

        let enabled = arguments.get("enabled").and_then(|v| v.as_bool());
        let new_cat = arguments.get("category").and_then(|v| v.as_str());

        if enabled.is_none() && new_cat.is_none() {
            return Err(AppError::Validation(
                "At least one of 'enabled' or 'category' must be provided".into(),
            ));
        }

        if let Some(cat) = new_cat
            && !VALID_CATEGORIES.contains(&cat)
        {
            return Err(AppError::Validation(format!(
                "Invalid category '{}'. Must be one of: {}",
                cat,
                VALID_CATEGORIES.join(", ")
            )));
        }

        let old = self
            .memory_repo
            .get_by_id(memory_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Memory {memory_id} not found")))?;

        let updated = self
            .memory_repo
            .update(memory_id, None, enabled, new_cat)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Memory {memory_id} not found")))?;

        let mut changes = Vec::new();
        if let Some(e) = enabled {
            changes.push(format!("enabled: {} → {}", old.enabled, e));
        }
        if let Some(c) = new_cat {
            changes.push(format!("category: {} → {}", old.category, c));
        }

        Ok(format!(
            "Memory {} updated.\nChanges: {}\nContent: {}",
            updated.id,
            changes.join(", "),
            updated.content,
        ))
    }
}
