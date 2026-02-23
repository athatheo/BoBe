use std::sync::Arc;
use tracing::{debug, info, warn};

use super::executor::ToolExecutor;
use crate::error::AppError;
use crate::ports::llm::LlmProvider;
use crate::ports::llm_types::{AiMessage, AiResponse, ToolDefinition};
use crate::ports::tools::{ToolExecutionContext, ToolExecutionNotification, ToolResult};

/// Configuration for the tool call loop.
#[derive(Debug, Clone)]
pub struct ToolCallLoopConfig {
    pub max_iterations: usize,
    pub timeout_per_tool_secs: f64,
}

impl Default for ToolCallLoopConfig {
    fn default() -> Self {
        Self {
            max_iterations: 5,
            timeout_per_tool_secs: 30.0,
        }
    }
}

/// Agentic loop: LLM → tool calls → results → LLM, until stop or max iterations.
pub struct ToolCallLoop {
    llm: Arc<dyn LlmProvider>,
    executor: Arc<ToolExecutor>,
    config: ToolCallLoopConfig,
}

impl ToolCallLoop {
    pub fn new(
        llm: Arc<dyn LlmProvider>,
        executor: Arc<ToolExecutor>,
        config: ToolCallLoopConfig,
    ) -> Self {
        Self {
            llm,
            executor,
            config,
        }
    }

    /// Run the non-streaming tool call loop.
    pub async fn run(
        &self,
        messages: &[AiMessage],
        tools: &[ToolDefinition],
        temperature: f32,
        max_tokens: u32,
        context: Option<&ToolExecutionContext>,
    ) -> Result<AiResponse, AppError> {
        let mut working_messages = messages.to_vec();

        for iteration in 0..self.config.max_iterations {
            debug!(iteration, "Tool call loop iteration");

            let response = self
                .llm
                .complete(
                    &working_messages,
                    Some(tools),
                    None,
                    temperature,
                    max_tokens,
                )
                .await?;

            if response.finish_reason != "tool_calls" || response.message.tool_calls.is_empty() {
                info!(
                    iteration,
                    finish_reason = %response.finish_reason,
                    "Tool call loop completed"
                );
                return Ok(response);
            }

            let tool_calls = &response.message.tool_calls;
            info!(
                iteration,
                tool_count = tool_calls.len(),
                "Executing tool calls"
            );

            // Add assistant message with tool calls
            working_messages.push(AiMessage::assistant_with_tool_calls(tool_calls.clone()));

            // Execute all tool calls
            let results = self.executor.execute_batch(tool_calls, context).await;

            // Append tool results as messages
            for result in &results {
                working_messages.push(AiMessage::tool(
                    result.tool_call_id.clone(),
                    result.tool_name.clone(),
                    result.content.clone(),
                ));
            }
        }

        // Max iterations reached — do a final call without tools
        warn!(
            max_iterations = self.config.max_iterations,
            "Tool call loop hit max iterations, making final call without tools"
        );

        let final_response = self
            .llm
            .complete(&working_messages, None, None, temperature, max_tokens)
            .await?;

        Ok(final_response)
    }

    /// Run the streaming tool call loop.
    ///
    /// Yields `StreamChunk` for text deltas and `ToolExecutionNotification` for tool events.
    pub async fn stream(
        &self,
        messages: &[AiMessage],
        tools: &[ToolDefinition],
        temperature: f32,
        max_tokens: u32,
        context: Option<&ToolExecutionContext>,
    ) -> Result<StreamingToolCallResult, AppError> {
        let mut working_messages = messages.to_vec();
        let mut all_notifications = Vec::new();

        for iteration in 0..self.config.max_iterations {
            debug!(iteration, "Streaming tool call loop iteration");

            let response = self
                .llm
                .complete(
                    &working_messages,
                    Some(tools),
                    None,
                    temperature,
                    max_tokens,
                )
                .await?;

            if response.finish_reason != "tool_calls" || response.message.tool_calls.is_empty() {
                // Final response — return for streaming
                return Ok(StreamingToolCallResult {
                    messages: working_messages,
                    final_tools: if iteration < self.config.max_iterations - 1 {
                        Some(tools.to_vec())
                    } else {
                        None
                    },
                    notifications: all_notifications,
                    temperature,
                    max_tokens,
                });
            }

            let tool_calls = &response.message.tool_calls;
            working_messages.push(AiMessage::assistant_with_tool_calls(tool_calls.clone()));

            // Execute tools with notifications
            let (notifications, results) = self
                .execute_tools_with_notifications(tool_calls, context)
                .await;
            all_notifications.extend(notifications);

            for result in &results {
                working_messages.push(AiMessage::tool(
                    result.tool_call_id.clone(),
                    result.tool_name.clone(),
                    result.content.clone(),
                ));
            }
        }

        warn!("Streaming loop hit max iterations");
        Ok(StreamingToolCallResult {
            messages: working_messages,
            final_tools: None,
            notifications: all_notifications,
            temperature,
            max_tokens,
        })
    }

    async fn execute_tools_with_notifications(
        &self,
        tool_calls: &[crate::ports::llm_types::AiToolCall],
        context: Option<&ToolExecutionContext>,
    ) -> (Vec<ToolExecutionNotification>, Vec<ToolResult>) {
        let mut notifications = Vec::new();
        let mut results = Vec::new();

        // Emit start notifications
        for tc in tool_calls {
            notifications.push(ToolExecutionNotification {
                notification_type: "tool_start".into(),
                tool_name: tc.name.clone(),
                tool_call_id: tc.id.clone(),
                success: None,
                error: None,
                duration_ms: None,
            });
        }

        // Execute in parallel
        let start = std::time::Instant::now();
        let batch_results = self.executor.execute_batch(tool_calls, context).await;

        // Emit completion notifications
        for result in &batch_results {
            let duration = start.elapsed();
            notifications.push(ToolExecutionNotification {
                notification_type: "tool_complete".into(),
                tool_name: result.tool_name.clone(),
                tool_call_id: result.tool_call_id.clone(),
                success: Some(result.success),
                error: result.error.clone(),
                duration_ms: Some(duration.as_secs_f64() * 1000.0),
            });
        }

        results.extend(batch_results);
        (notifications, results)
    }
}

/// Result of a streaming tool call loop — used to feed the final LLM streaming call.
#[derive(Debug)]
pub struct StreamingToolCallResult {
    pub messages: Vec<AiMessage>,
    pub final_tools: Option<Vec<ToolDefinition>>,
    pub notifications: Vec<ToolExecutionNotification>,
    pub temperature: f32,
    pub max_tokens: u32,
}
