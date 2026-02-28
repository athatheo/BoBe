pub mod executor;
pub mod mcp;
pub mod native;
pub mod preselector;
pub mod registry;
pub mod tool_call_loop;

// ─── Types and trait definitions ────────────────────────────────────────────

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::AppError;
use crate::llm::types::{AiToolCall, ToolDefinition};

/// Result from tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub tool_name: String,
    pub success: bool,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ToolResult {
    pub fn ok(tool_call_id: String, tool_name: String, content: String) -> Self {
        Self {
            tool_call_id,
            tool_name,
            success: true,
            content,
            data: None,
            error: None,
        }
    }

    pub fn err(tool_call_id: String, tool_name: String, error: String) -> Self {
        Self {
            tool_call_id,
            tool_name: tool_name.clone(),
            success: false,
            content: format!("Error executing {tool_name}: {error}"),
            data: None,
            error: Some(error),
        }
    }
}

/// Context passed to tools during execution.
#[derive(Debug, Clone, Default)]
pub struct ToolExecutionContext {
    pub conversation_id: Option<String>,
}

/// Typed notification about tool execution lifecycle events.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolNotification {
    Started {
        tool_name: String,
        tool_call_id: String,
    },
    Completed {
        tool_name: String,
        tool_call_id: String,
        success: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
        duration_ms: f64,
    },
}

/// Protocol for tool sources.
///
/// Both BoBe native tools and MCP tools implement this trait.
#[async_trait]
pub trait ToolSource: Send + Sync {
    /// Source name (e.g., "bobe", "mcp:filesystem").
    fn name(&self) -> &str;

    /// Get available tool definitions.
    async fn get_tools(&self, include_disabled: bool) -> Result<Vec<ToolDefinition>, AppError>;

    /// Execute a tool call.
    async fn execute(
        &self,
        tool_call: &AiToolCall,
        context: Option<&ToolExecutionContext>,
    ) -> ToolResult;
}
