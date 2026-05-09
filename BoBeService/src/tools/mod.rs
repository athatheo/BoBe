pub(crate) mod mcp;
pub(crate) mod native;
pub(crate) mod registry;

// ─── Deprecated: superseded by Copilot SDK's built-in tool dispatch ─────────
//
// `ToolExecutor`, `ToolPreselector`, and `ToolCallLoop` predate the
// Copilot SDK pivot. The SDK's session loop (autopilot mode) handles
// tool selection + invocation natively via Copilot's built-in tools
// (Read, Write, Bash, Grep, Edit, etc.), so BoBe-side dispatch is no
// longer needed. The modules remain in-tree for reference but are not
// constructed at boot — see `bootstrap/wiring.rs`. Slated for removal
// alongside the rest of the LlmProvider consumer migration.

#[deprecated(note = "superseded by Copilot SDK session loop")]
pub(crate) mod executor;
#[deprecated(note = "superseded by Copilot SDK session loop")]
pub(crate) mod preselector;
#[deprecated(note = "superseded by Copilot SDK session loop")]
pub(crate) mod tool_call_loop;

// ─── Types and trait definitions ────────────────────────────────────────────

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
    pub(crate) conversation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ToolNotification {
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

#[async_trait]
pub(crate) trait ToolSource: Send + Sync {
    fn name(&self) -> &str;
    async fn get_tools(&self) -> Result<Vec<ToolDefinition>, AppError>;
    async fn execute(
        &self,
        tool_call: &AiToolCall,
        context: Option<&ToolExecutionContext>,
    ) -> ToolResult;
}
