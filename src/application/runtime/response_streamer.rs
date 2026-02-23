//! Response streamer — streams LLM responses through SSE events.
//!
//! Handles text deltas, tool notifications, error recovery.

use std::time::Instant;

use futures::StreamExt;
use tracing::error;
use uuid::Uuid;

use crate::adapters::sse::event_queue::EventQueue;
use crate::adapters::sse::types::{EventType, StreamBundle};
use crate::error::AppError;
use crate::ports::llm_types::StreamChunk;

/// Result of streaming an LLM response.
#[derive(Debug)]
pub struct StreamResult {
    pub full_response: String,
    pub token_count: usize,
    pub duration_ms: f64,
    pub success: bool,
    pub error: Option<String>,
}

/// Stream LLM response chunks to SSE event queue.
pub async fn stream_llm_response(
    mut stream: std::pin::Pin<Box<dyn futures::Stream<Item = Result<StreamChunk, AppError>> + Send + '_>>,
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

    while let Some(item) = stream.next().await {
        match item {
            Ok(chunk) => {
                if !chunk.delta.is_empty() {
                    full_response.push_str(&chunk.delta);

                    let event = StreamBundle {
                        event_type: EventType::TextDelta,
                        message_id: msg_id.clone(),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        description: "text_delta".into(),
                        payload: serde_json::json!({
                            "delta": chunk.delta,
                            "sequence": sequence,
                            "done": false,
                        }),
                    };
                    event_queue.push(event);
                    sequence += 1;
                }

                if chunk.finish_reason.is_some() {
                    break;
                }
            }
            Err(e) => {
                success = false;
                error_msg = Some(e.to_string());
                error!(error = %e, chunks = sequence, "stream_response.error");

                let event = StreamBundle {
                    event_type: EventType::Error,
                    message_id: msg_id.clone(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    description: "stream_error".into(),
                    payload: serde_json::json!({
                        "code": "RESPONSE_ERROR",
                        "message": e.to_string(),
                        "recoverable": true,
                    }),
                };
                event_queue.push(event);
                break;
            }
        }
    }

    // Send final done event
    let done_event = StreamBundle {
        event_type: EventType::TextDelta,
        message_id: msg_id,
        timestamp: chrono::Utc::now().to_rfc3339(),
        description: "text_delta".into(),
        payload: serde_json::json!({
            "delta": "",
            "sequence": sequence,
            "done": true,
        }),
    };
    event_queue.push(done_event);

    let duration_ms = start_time.elapsed().as_secs_f64() * 1000.0;

    StreamResult {
        full_response,
        token_count: sequence,
        duration_ms,
        success,
        error: error_msg,
    }
}

/// Stream a simple text message (no LLM call needed).
pub fn stream_simple_message(
    message: &str,
    event_queue: &EventQueue,
    msg_id: Option<&str>,
) {
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
