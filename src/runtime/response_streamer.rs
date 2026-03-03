//! Response streamer — streams LLM responses through SSE events.
//!
//! Handles text deltas, tool notifications, error recovery.

use std::time::Instant;

use futures::StreamExt;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::error::AppError;
use crate::llm::types::{StreamChunk, StreamItem};
use crate::util::sse::event_queue::EventQueue;
use crate::util::sse::factories::{
    end_of_turn_event, error_event, text_delta_event, tool_call_complete_event,
    tool_call_start_event,
};

/// Result of streaming an LLM response.
#[derive(Debug)]
pub struct StreamResult {
    pub full_response: String,
    pub chunk_count: usize,
    pub duration_ms: f64,
    pub success: bool,
    pub first_token_ms: Option<f64>,
}

/// Stream a mixed LLM + tool notification stream to SSE event queue.
///
/// Handles both `StreamItem::Chunk` (text deltas) and
/// `StreamItem::TypedToolNotification` (tool execution start/complete events).
/// This is the primary streaming function used when tools are enabled.
pub async fn stream_response(
    mut stream: std::pin::Pin<
        Box<dyn futures::Stream<Item = Result<StreamItem, AppError>> + Send + '_>,
    >,
    event_queue: &EventQueue,
    msg_id: Option<&str>,
) -> StreamResult {
    let msg_id = msg_id.map_or_else(|| format!("msg_{}", Uuid::new_v4().simple()), str::to_owned);

    let start_time = Instant::now();
    let mut sequence = 0usize;
    let mut full_response = String::new();
    let mut success = true;
    let mut first_token_time: Option<Instant> = None;

    while let Some(item) = stream.next().await {
        match item {
            Ok(StreamItem::Chunk(chunk)) => {
                if first_token_time.is_none() && !chunk.delta.is_empty() {
                    first_token_time = Some(Instant::now());
                }
                if !handle_stream_chunk(
                    &chunk,
                    &msg_id,
                    &mut sequence,
                    &mut full_response,
                    event_queue,
                ) {
                    break;
                }
            }
            Ok(StreamItem::TypedToolNotification(notification)) => {
                handle_tool_notification(&notification, &msg_id, event_queue);
            }
            Err(e) => {
                success = false;
                error!(error = %e, chunks = sequence, "stream_response.error");
                // Classify error: tool system errors are non-recoverable
                let (code, recoverable) = classify_error(&e);
                push_error_event_classified(&msg_id, &e, code, recoverable, event_queue);
                break;
            }
        }
    }

    push_done_event(&msg_id, sequence, event_queue);

    let duration_ms = start_time.elapsed().as_secs_f64() * 1000.0;
    let first_token_ms = first_token_time.map(|t| (t - start_time).as_secs_f64() * 1000.0);

    StreamResult {
        full_response,
        chunk_count: sequence,
        duration_ms,
        success,
        first_token_ms,
    }
}

/// Stream LLM response chunks (no tool notifications) to SSE event queue.
///
/// Used when tools are disabled — the stream only contains `StreamChunk` items.
pub async fn stream_llm_response(
    mut stream: std::pin::Pin<
        Box<dyn futures::Stream<Item = Result<StreamChunk, AppError>> + Send + '_>,
    >,
    event_queue: &EventQueue,
    msg_id: Option<&str>,
) -> StreamResult {
    let msg_id = msg_id.map_or_else(|| format!("msg_{}", Uuid::new_v4().simple()), str::to_owned);

    let start_time = Instant::now();
    let mut sequence = 0usize;
    let mut full_response = String::new();
    let mut success = true;
    let mut first_token_time: Option<Instant> = None;

    while let Some(item) = stream.next().await {
        match item {
            Ok(chunk) => {
                if first_token_time.is_none() && !chunk.delta.is_empty() {
                    first_token_time = Some(Instant::now());
                }
                if !handle_stream_chunk(
                    &chunk,
                    &msg_id,
                    &mut sequence,
                    &mut full_response,
                    event_queue,
                ) {
                    break;
                }
            }
            Err(e) => {
                success = false;
                error!(error = %e, chunks = sequence, "stream_response.error");
                push_error_event(&msg_id, &e, event_queue);
                break;
            }
        }
    }

    push_done_event(&msg_id, sequence, event_queue);

    let duration_ms = start_time.elapsed().as_secs_f64() * 1000.0;
    let first_token_ms = first_token_time.map(|t| (t - start_time).as_secs_f64() * 1000.0);

    StreamResult {
        full_response,
        chunk_count: sequence,
        duration_ms,
        success,
        first_token_ms,
    }
}

/// Handle a stream chunk — push text delta event, return false if stream should stop.
fn handle_stream_chunk(
    chunk: &StreamChunk,
    msg_id: &str,
    sequence: &mut usize,
    full_response: &mut String,
    event_queue: &EventQueue,
) -> bool {
    if !chunk.delta.is_empty() {
        full_response.push_str(&chunk.delta);
        event_queue.push(text_delta_event(msg_id, &chunk.delta, *sequence, false));
        *sequence += 1;
    }

    // Don't stop on finish_reason — the tool call loop may continue with more iterations
    // The stream itself will end when the loop is done.
    true
}

/// Handle a tool execution notification — push start or complete event.
fn handle_tool_notification(
    notification: &crate::tools::ToolNotification,
    msg_id: &str,
    event_queue: &EventQueue,
) {
    match notification {
        crate::tools::ToolNotification::Started {
            tool_name,
            tool_call_id,
        } => {
            info!(tool = %tool_name, "tool_call.start");
            event_queue.push(tool_call_start_event(msg_id, tool_name, tool_call_id));
            debug!(tool = %tool_name, tool_call_id = %tool_call_id, "tool_call.typed_start");
        }
        crate::tools::ToolNotification::Completed {
            tool_name,
            tool_call_id,
            success,
            error,
            duration_ms,
        } => {
            info!(tool = %tool_name, success, duration_ms, "tool_call.complete");
            event_queue.push(tool_call_complete_event(
                msg_id,
                tool_name,
                tool_call_id,
                Some(*success),
                error.as_deref(),
                Some(*duration_ms),
            ));
            debug!(
                tool = %tool_name,
                tool_call_id = %tool_call_id,
                success,
                error = ?error,
                duration_ms,
                "tool_call.typed_complete"
            );
        }
    }
}

fn push_error_event(msg_id: &str, error: &AppError, event_queue: &EventQueue) {
    let (code, recoverable) = classify_error(error);
    push_error_event_classified(msg_id, error, code, recoverable, event_queue);
}

fn push_error_event_classified(
    msg_id: &str,
    error: &AppError,
    code: &str,
    recoverable: bool,
    event_queue: &EventQueue,
) {
    event_queue.push(error_event(msg_id, code, &error.to_string(), recoverable));
}

/// Classify an error for SSE reporting.
/// Tool system errors are non-recoverable; LLM errors are recoverable.
fn classify_error(error: &AppError) -> (&'static str, bool) {
    match error {
        AppError::Tool(_) => ("TOOL_SYSTEM_ERROR", false),
        AppError::LlmUnavailable(_) => ("LLM_UNAVAILABLE", false),
        AppError::LlmTimeout(_) => ("LLM_TIMEOUT", true),
        _ => ("RESPONSE_ERROR", true),
    }
}

fn push_done_event(msg_id: &str, sequence: usize, event_queue: &EventQueue) {
    event_queue.push(end_of_turn_event(msg_id, sequence));
}

/// Stream a simple text message (no LLM call needed).
pub fn stream_simple_message(message: &str, event_queue: &EventQueue, msg_id: Option<&str>) {
    let msg_id = msg_id.map_or_else(|| format!("msg_{}", Uuid::new_v4().simple()), str::to_owned);

    event_queue.push(text_delta_event(&msg_id, message, 0, false));
    event_queue.push(end_of_turn_event(&msg_id, 1));
}
