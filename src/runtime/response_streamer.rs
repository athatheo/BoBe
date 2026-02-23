//! Response streamer — streams LLM responses through SSE events.
//!
//! Handles text deltas, tool notifications, error recovery.

use std::time::Instant;

use futures::StreamExt;
use tracing::{error, info};
use uuid::Uuid;

use crate::util::sse::event_queue::EventQueue;
use crate::util::sse::types::{EventType, StreamBundle};
use crate::error::AppError;
use crate::llm::types::{StreamChunk, StreamItem};

/// Result of streaming an LLM response.
#[derive(Debug)]
#[allow(dead_code)]
pub struct StreamResult {
    pub full_response: String,
    pub token_count: usize,
    pub duration_ms: f64,
    pub success: bool,
    pub error: Option<String>,
    pub first_token_ms: Option<f64>,
}

/// Stream a mixed LLM + tool notification stream to SSE event queue.
///
/// Handles both `StreamItem::Chunk` (text deltas) and `StreamItem::ToolNotification`
/// (tool execution start/complete events). This is the primary streaming function
/// used when tools are enabled.
pub async fn stream_response(
    mut stream: std::pin::Pin<
        Box<dyn futures::Stream<Item = Result<StreamItem, AppError>> + Send + '_>,
    >,
    event_queue: &EventQueue,
    msg_id: Option<&str>,
) -> StreamResult {
    let msg_id = msg_id
        .map(|s| s.to_owned())
        .unwrap_or_else(|| format!("msg_{}", Uuid::new_v4().simple()));

    let start_time = Instant::now();
    let mut sequence = 0usize;
    let mut full_response = String::new();
    let mut error_msg: Option<String> = None;
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
            Ok(StreamItem::ToolNotification(notification)) => {
                handle_tool_notification(&notification, &msg_id, event_queue);
            }
            Err(e) => {
                success = false;
                error_msg = Some(e.to_string());
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
        token_count: sequence,
        duration_ms,
        success,
        error: error_msg,
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
    let msg_id = msg_id
        .map(|s| s.to_owned())
        .unwrap_or_else(|| format!("msg_{}", Uuid::new_v4().simple()));

    let start_time = Instant::now();
    let mut sequence = 0usize;
    let mut full_response = String::new();
    let mut error_msg: Option<String> = None;
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
                error_msg = Some(e.to_string());
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
        token_count: sequence,
        duration_ms,
        success,
        error: error_msg,
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

        let event = StreamBundle {
            event_type: EventType::TextDelta,
            message_id: msg_id.to_owned(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            description: "text_delta".into(),
            payload: serde_json::json!({
                "delta": chunk.delta,
                "sequence": *sequence,
                "done": false,
            }),
        };
        event_queue.push(event);
        *sequence += 1;
    }

    // Don't stop on finish_reason — the tool call loop may continue with more iterations
    // The stream itself will end when the loop is done.
    true
}

/// Handle a tool execution notification — push start or complete event.
fn handle_tool_notification(
    notification: &crate::tools::ToolExecutionNotification,
    msg_id: &str,
    event_queue: &EventQueue,
) {
    if notification.notification_type == "start" {
        info!(tool = %notification.tool_name, "tool_call.start");
        event_queue.push(StreamBundle {
            event_type: EventType::ToolCallStart,
            message_id: msg_id.to_owned(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            description: "tool_call_start".into(),
            payload: serde_json::json!({
                "tool_name": notification.tool_name,
                "tool_call_id": notification.tool_call_id,
            }),
        });
    } else {
        info!(
            tool = %notification.tool_name,
            success = ?notification.success,
            duration_ms = ?notification.duration_ms,
            "tool_call.complete"
        );
        event_queue.push(StreamBundle {
            event_type: EventType::ToolCallComplete,
            message_id: msg_id.to_owned(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            description: "tool_call_complete".into(),
            payload: serde_json::json!({
                "tool_name": notification.tool_name,
                "tool_call_id": notification.tool_call_id,
                "success": notification.success,
                "error": notification.error,
                "duration_ms": notification.duration_ms,
            }),
        });
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
    event_queue.push(StreamBundle {
        event_type: EventType::Error,
        message_id: msg_id.to_owned(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        description: "stream_error".into(),
        payload: serde_json::json!({
            "code": code,
            "message": error.to_string(),
            "recoverable": recoverable,
        }),
    });
}

/// Classify an error for SSE reporting.
/// Tool system errors are non-recoverable; LLM errors are recoverable.
fn classify_error(error: &AppError) -> (&'static str, bool) {
    let msg = error.to_string().to_lowercase();
    if msg.contains("tool") {
        ("TOOL_SYSTEM_ERROR", false)
    } else if msg.contains("timeout") {
        ("LLM_TIMEOUT", true)
    } else {
        ("RESPONSE_ERROR", true)
    }
}

fn push_done_event(msg_id: &str, sequence: usize, event_queue: &EventQueue) {
    event_queue.push(StreamBundle {
        event_type: EventType::TextDelta,
        message_id: msg_id.to_owned(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        description: "text_delta".into(),
        payload: serde_json::json!({
            "delta": "",
            "sequence": sequence,
            "done": true,
        }),
    });
}

/// Stream a simple text message (no LLM call needed).
#[allow(dead_code)]
pub fn stream_simple_message(message: &str, event_queue: &EventQueue, msg_id: Option<&str>) {
    let msg_id = msg_id
        .map(|s| s.to_owned())
        .unwrap_or_else(|| format!("msg_{}", Uuid::new_v4().simple()));

    event_queue.push(StreamBundle {
        event_type: EventType::TextDelta,
        message_id: msg_id.clone(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        description: "text_delta".into(),
        payload: serde_json::json!({
            "delta": message,
            "sequence": 0,
            "done": false,
        }),
    });

    event_queue.push(StreamBundle {
        event_type: EventType::TextDelta,
        message_id: msg_id,
        timestamp: chrono::Utc::now().to_rfc3339(),
        description: "text_delta".into(),
        payload: serde_json::json!({
            "delta": "",
            "sequence": 1,
            "done": true,
        }),
    });
}
