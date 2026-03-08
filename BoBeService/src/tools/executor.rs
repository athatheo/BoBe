use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info, warn};

use super::registry::ToolRegistry;
use crate::llm::types::AiToolCall;
use crate::tools::{ToolExecutionContext, ToolResult};

pub(crate) struct ToolExecutor {
    registry: Arc<ToolRegistry>,
    default_timeout: std::time::Duration,
}

impl ToolExecutor {
    pub(crate) fn new(registry: Arc<ToolRegistry>, default_timeout_secs: f64) -> Self {
        Self {
            registry,
            default_timeout: std::time::Duration::from_secs_f64(default_timeout_secs),
        }
    }

    pub(crate) async fn execute(
        &self,
        tool_call: &AiToolCall,
        timeout: Option<std::time::Duration>,
        context: Option<&ToolExecutionContext>,
    ) -> ToolResult {
        let start = Instant::now();
        let timeout = timeout.unwrap_or(self.default_timeout);

        if let Some(false) = self.registry.is_tool_enabled(&tool_call.name) {
            warn!(tool = %tool_call.name, "Attempted to execute disabled tool");
            return ToolResult::err(
                tool_call.id.clone(),
                tool_call.name.clone(),
                format!("Tool '{}' is disabled", tool_call.name),
            );
        }

        let Some(source) = self.registry.get_source_for_tool(&tool_call.name).await else {
            warn!(tool = %tool_call.name, "No source found for tool");
            return ToolResult::err(
                tool_call.id.clone(),
                tool_call.name.clone(),
                format!("Unknown tool: '{}'", tool_call.name),
            );
        };

        debug!(
            tool = %tool_call.name,
            source = %source.name(),
            timeout_ms = timeout.as_millis(),
            "Executing tool"
        );

        let result = tokio::time::timeout(timeout, source.execute(tool_call, context)).await;

        let duration = start.elapsed();

        if let Ok(tool_result) = result {
            info!(
                tool = %tool_call.name,
                success = tool_result.success,
                duration_ms = duration.as_millis(),
                content_len = tool_result.content.len(),
                "Tool execution completed"
            );
            tool_result
        } else {
            warn!(
                tool = %tool_call.name,
                timeout_ms = timeout.as_millis(),
                "Tool execution timed out"
            );
            ToolResult::err(
                tool_call.id.clone(),
                tool_call.name.clone(),
                format!(
                    "Tool execution timed out after {:.1}s",
                    timeout.as_secs_f64()
                ),
            )
        }
    }

    pub(crate) async fn execute_batch(
        &self,
        tool_calls: &[AiToolCall],
        context: Option<&ToolExecutionContext>,
    ) -> Vec<ToolResult> {
        let futures: Vec<_> = tool_calls
            .iter()
            .map(|tc| self.execute(tc, None, context))
            .collect();

        futures::future::join_all(futures).await
    }
}
