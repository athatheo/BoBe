use std::pin::Pin;
use std::sync::Arc;

use futures::Stream;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, info, warn};

use super::executor::ToolExecutor;
use crate::error::AppError;
use crate::ports::llm::LlmProvider;
use crate::ports::llm_types::{AiMessage, AiResponse, StreamItem, ToolDefinition};
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

            working_messages.push(AiMessage::assistant_with_tool_calls(tool_calls.clone()));

            let results = self.executor.execute_batch(tool_calls, context).await;

            for result in &results {
                working_messages.push(AiMessage::tool(
                    result.tool_call_id.clone(),
                    result.tool_name.clone(),
                    result.content.clone(),
                ));
            }
        }

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
    /// Returns a stream that yields `StreamItem` — either LLM text chunks or
    /// tool execution notifications. The stream handles the full agentic loop:
    /// stream LLM → detect tool calls → execute tools (with notifications) →
    /// feed results back → stream next LLM response.
    pub fn stream(
        &self,
        messages: Vec<AiMessage>,
        tools: Vec<ToolDefinition>,
        temperature: f32,
        max_tokens: u32,
        context: Option<ToolExecutionContext>,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamItem, AppError>> + Send>> {
        let (tx, rx) = mpsc::channel::<Result<StreamItem, AppError>>(64);

        let llm = self.llm.clone();
        let executor = self.executor.clone();
        let max_iterations = self.config.max_iterations;

        tokio::spawn(async move {
            if let Err(e) = run_streaming_loop(
                llm, executor, tx.clone(), messages, tools,
                temperature, max_tokens, max_iterations, context,
            ).await {
                let _ = tx.send(Err(e)).await;
            }
        });

        Box::pin(ReceiverStream::new(rx))
    }

    async fn execute_tools_with_notifications(
        &self,
        tool_calls: &[crate::ports::llm_types::AiToolCall],
        context: Option<&ToolExecutionContext>,
    ) -> (Vec<ToolExecutionNotification>, Vec<ToolResult>) {
        execute_tools_with_notifications(&self.executor, tool_calls, context).await
    }
}

/// Internal: run the streaming tool call loop, sending items through the channel.
async fn run_streaming_loop(
    llm: Arc<dyn LlmProvider>,
    executor: Arc<ToolExecutor>,
    tx: mpsc::Sender<Result<StreamItem, AppError>>,
    messages: Vec<AiMessage>,
    tools: Vec<ToolDefinition>,
    temperature: f32,
    max_tokens: u32,
    max_iterations: usize,
    context: Option<ToolExecutionContext>,
) -> Result<(), AppError> {
    use futures::StreamExt;

    let mut current_messages = messages;
    let ctx_ref = context.as_ref();

    for iteration in 0..max_iterations {
        debug!(iteration, "Streaming tool call loop iteration");

        // Withhold tools on the final iteration to force a text response
        let current_tools = if iteration < max_iterations - 1 {
            Some(&tools[..])
        } else {
            None
        };

        let mut llm_stream = llm.stream(
            current_messages.clone(),
            current_tools.map(|t| t.to_vec()),
            None,
            temperature,
            max_tokens,
        );

        let mut accumulated_content = String::new();
        let mut accumulated_tool_calls = Vec::new();
        let mut finish_reason: Option<String> = None;

        while let Some(chunk_result) = llm_stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    if !chunk.delta.is_empty() {
                        accumulated_content.push_str(&chunk.delta);
                    }
                    if !chunk.tool_calls.is_empty() {
                        accumulated_tool_calls.extend(chunk.tool_calls.clone());
                    }
                    if chunk.finish_reason.is_some() {
                        finish_reason = chunk.finish_reason.clone();
                    }
                    // Yield text deltas immediately
                    if tx.send(Ok(StreamItem::Chunk(chunk))).await.is_err() {
                        return Ok(()); // receiver dropped
                    }
                }
                Err(e) => {
                    let _ = tx.send(Err(e)).await;
                    return Ok(());
                }
            }
        }

        // Check if done (no tool calls requested)
        if finish_reason.as_deref() != Some("tool_calls") || accumulated_tool_calls.is_empty() {
            info!(
                iteration,
                finish_reason = ?finish_reason,
                "Streaming tool call loop completed"
            );
            return Ok(());
        }

        // Add assistant message with accumulated content + tool calls
        let mut assistant_msg = AiMessage::assistant_with_tool_calls(accumulated_tool_calls.clone());
        if !accumulated_content.is_empty() {
            assistant_msg.content = crate::ports::llm_types::MessageContent::Text(accumulated_content);
        }
        current_messages.push(assistant_msg);

        // Execute tools with notifications
        let (notifications, results) =
            execute_tools_with_notifications(&executor, &accumulated_tool_calls, ctx_ref).await;

        // Yield all notifications
        for notification in notifications {
            if tx.send(Ok(StreamItem::ToolNotification(notification))).await.is_err() {
                return Ok(());
            }
        }

        // Add tool results to messages
        for result in &results {
            current_messages.push(AiMessage::tool(
                result.tool_call_id.clone(),
                result.tool_name.clone(),
                result.content.clone(),
            ));
        }

        // Continue to next iteration (will stream next LLM response)
    }

    // Max iterations reached — stream final response without tools
    warn!("Streaming tool call loop hit max iterations");

    let mut final_stream = llm.stream(
        current_messages,
        None,
        None,
        temperature,
        max_tokens,
    );

    let mut final_has_tool_calls = false;
    while let Some(chunk_result) = final_stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                if !chunk.tool_calls.is_empty() {
                    final_has_tool_calls = true;
                }
                if tx.send(Ok(StreamItem::Chunk(chunk))).await.is_err() {
                    return Ok(());
                }
            }
            Err(e) => {
                let _ = tx.send(Err(e)).await;
                return Ok(());
            }
        }
    }

    if final_has_tool_calls {
        warn!("LLM still requesting tools after max iterations on final response");
    }

    Ok(())
}

/// Execute tools in parallel, returning start/complete notifications and results.
/// Each tool is tracked individually with its own error handling and duration.
async fn execute_tools_with_notifications(
    executor: &ToolExecutor,
    tool_calls: &[crate::ports::llm_types::AiToolCall],
    context: Option<&ToolExecutionContext>,
) -> (Vec<ToolExecutionNotification>, Vec<ToolResult>) {
    let mut notifications = Vec::new();

    // Emit start notifications
    for tc in tool_calls {
        notifications.push(ToolExecutionNotification {
            notification_type: "start".into(),
            tool_name: tc.name.clone(),
            tool_call_id: tc.id.clone(),
            success: None,
            error: None,
            duration_ms: None,
        });
    }

    // Execute in parallel, tracking per-tool timing
    let start_times: Vec<std::time::Instant> = tool_calls.iter().map(|_| std::time::Instant::now()).collect();
    let batch_results = executor.execute_batch(tool_calls, context).await;

    // Emit completion notifications with per-tool duration
    for (i, result) in batch_results.iter().enumerate() {
        let duration_ms = if i < start_times.len() {
            start_times[i].elapsed().as_secs_f64() * 1000.0
        } else {
            0.0
        };

        if !result.success {
            warn!(
                tool = %result.tool_name,
                error = ?result.error,
                duration_ms = format!("{duration_ms:.1}"),
                "tool_call.execution_failed"
            );
        }

        notifications.push(ToolExecutionNotification {
            notification_type: "complete".into(),
            tool_name: result.tool_name.clone(),
            tool_call_id: result.tool_call_id.clone(),
            success: Some(result.success),
            error: result.error.clone(),
            duration_ms: Some(duration_ms),
        });
    }

    (notifications, batch_results)
}
