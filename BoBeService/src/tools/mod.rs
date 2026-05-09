//! MCP server config parsing + adapter (lifecycle management of MCP
//! processes). Phase 5-mcp will fold MCP server registration into the
//! Copilot SDK via `SessionConfig::mcp_servers`; this module survives
//! until then to keep `/api/tools/mcp/config` operational.

pub(crate) mod mcp;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::AppError;
use crate::llm::types::{AiToolCall, ToolDefinition};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ToolResult {
    pub(crate) tool_call_id: String,
    pub(crate) tool_name: String,
    pub(crate) success: bool,
    pub(crate) content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) data: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}

impl ToolResult {
    #[allow(dead_code, reason = "kept for McpToolAdapter; goes with Phase 5-mcp")]
    pub(crate) fn ok(tool_call_id: String, tool_name: String, content: String) -> Self {
        Self {
            tool_call_id,
            tool_name,
            success: true,
            content,
            data: None,
            error: None,
        }
    }

    #[allow(dead_code, reason = "kept for McpToolAdapter; goes with Phase 5-mcp")]
    pub(crate) fn err(tool_call_id: String, tool_name: String, error: String) -> Self {
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

#[derive(Debug, Clone, Default)]
pub(crate) struct ToolExecutionContext {
    #[allow(dead_code, reason = "kept for McpToolAdapter; goes with Phase 5-mcp")]
    pub(crate) conversation_id: Option<String>,
}

#[async_trait]
#[allow(dead_code, reason = "kept for McpToolAdapter; goes with Phase 5-mcp")]
pub(crate) trait ToolSource: Send + Sync {
    fn name(&self) -> &str;
    async fn get_tools(&self) -> Result<Vec<ToolDefinition>, AppError>;
    async fn execute(
        &self,
        tool_call: &AiToolCall,
        context: Option<&ToolExecutionContext>,
    ) -> ToolResult;
}
