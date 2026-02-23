use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

use crate::error::AppError;
use crate::tools::ToolCategory;

/// Base trait for all BoBe native tools.
///
/// Each tool provides metadata (name, description, JSON Schema parameters)
/// and an async execute method that processes arguments and returns a string result.
#[async_trait]
pub trait NativeTool: Send + Sync {
    /// Unique tool name (e.g., "search_memories").
    fn name(&self) -> &str;

    /// Human-readable description for the LLM.
    fn description(&self) -> &str;

    /// JSON Schema describing accepted parameters.
    fn parameters(&self) -> Value;

    /// Tool category for filtering/grouping.
    fn category(&self) -> ToolCategory;

    /// Execute the tool with the given arguments.
    async fn execute(
        &self,
        arguments: HashMap<String, Value>,
        context: Option<&crate::tools::ToolExecutionContext>,
    ) -> Result<String, AppError>;
}
