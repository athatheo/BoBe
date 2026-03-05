use std::pin::Pin;
use std::sync::Arc;

use arc_swap::ArcSwap;
use futures::Stream;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, info, warn};

use super::executor::ToolExecutor;
use crate::config::Config;
use crate::constants::MILLIS_PER_SECOND;
use crate::error::AppError;
use crate::llm::LlmProvider;
use crate::llm::types::{AiMessage, StreamItem, ToolDefinition};
use crate::tools::{ToolExecutionContext, ToolNotification, ToolResult};
use crate::util::tokens::{clamp_max_tokens, count_message_tokens};

pub(crate) struct ToolCallLoop {
    llm: Arc<dyn LlmProvider>,
    executor: Arc<ToolExecutor>,
    config: Arc<ArcSwap<Config>>,
}

impl ToolCallLoop {
    pub(crate) fn new(
        llm: Arc<dyn LlmProvider>,
        executor: Arc<ToolExecutor>,
        config: Arc<ArcSwap<Config>>,
    ) -> Self {
        Self {
            llm,
            executor,
            config,
        }
    }

    pub(crate) fn stream(
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
        let cfg = self.config.load();
        let max_iterations = cfg.tools.max_iterations as usize;

        let prompt_tokens = count_message_tokens(&messages);
        let max_tokens = clamp_max_tokens(cfg.llm.context_window, prompt_tokens, max_tokens);

        tokio::spawn(async move {
            if let Err(e) = run_streaming_loop(
                llm,
                executor,
                tx.clone(),
                messages,
                tools,
                temperature,
                max_tokens,
                max_iterations,
                context,
            )
            .await
            {
                let _ = tx.send(Err(e)).await;
            }
        });

        Box::pin(ReceiverStream::new(rx))
    }
}

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

        // Withhold tools on final iteration to force a text response
        let current_tools = if iteration < max_iterations - 1 {
            Some(&tools[..])
        } else {
            None
        };

        let mut llm_stream = llm.stream(
            current_messages.clone(),
            current_tools.map(<[ToolDefinition]>::to_vec),
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
                        accumulated_tool_calls.extend(chunk.tool_calls.iter().cloned());
                    }
                    if let Some(ref reason) = chunk.finish_reason {
                        finish_reason = Some(reason.clone());
                    }
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

        if finish_reason.as_deref() != Some("tool_calls") || accumulated_tool_calls.is_empty() {
            info!(
                iteration,
                finish_reason = ?finish_reason,
                "Streaming tool call loop completed"
            );
            return Ok(());
        }

        let mut assistant_msg =
            AiMessage::assistant_with_tool_calls(accumulated_tool_calls.clone());
        if !accumulated_content.is_empty() {
            assistant_msg.content = crate::llm::types::MessageContent::Text(accumulated_content);
        }
        current_messages.push(assistant_msg);

        let (typed_notifications, results) =
            execute_tools_with_notifications(&executor, &accumulated_tool_calls, ctx_ref).await;

        for typed in typed_notifications {
            if tx
                .send(Ok(StreamItem::TypedToolNotification(typed)))
                .await
                .is_err()
            {
                return Ok(());
            }
        }

        for result in &results {
            current_messages.push(AiMessage::tool(
                result.tool_call_id.clone(),
                result.tool_name.clone(),
                result.content.clone(),
            ));
        }
    }

    warn!("Streaming tool call loop hit max iterations");

    let mut final_stream = llm.stream(current_messages, None, None, temperature, max_tokens);

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

async fn execute_tools_with_notifications(
    executor: &ToolExecutor,
    tool_calls: &[crate::llm::types::AiToolCall],
    context: Option<&ToolExecutionContext>,
) -> (Vec<ToolNotification>, Vec<ToolResult>) {
    let mut typed_notifications = Vec::new();

    for tc in tool_calls {
        typed_notifications.push(ToolNotification::Started {
            tool_name: tc.name.clone(),
            tool_call_id: tc.id.clone(),
        });
    }

    let start_times: Vec<std::time::Instant> = tool_calls
        .iter()
        .map(|_| std::time::Instant::now())
        .collect();
    let batch_results = executor.execute_batch(tool_calls, context).await;

    for (i, result) in batch_results.iter().enumerate() {
        let duration_ms = if i < start_times.len() {
            start_times[i].elapsed().as_secs_f64() * MILLIS_PER_SECOND
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

        typed_notifications.push(ToolNotification::Completed {
            tool_name: result.tool_name.clone(),
            tool_call_id: result.tool_call_id.clone(),
            success: result.success,
            error: result.error.clone(),
            duration_ms,
        });
    }

    (typed_notifications, batch_results)
}
