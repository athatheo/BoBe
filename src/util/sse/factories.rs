use serde_json::json;

use super::types::{EventType, IndicatorType, StreamBundle};

/// Create an indicator event showing the daemon's current activity.
pub fn indicator_event(indicator: IndicatorType, message: Option<&str>) -> StreamBundle {
    indicator_event_with_progress(indicator, message, None)
}

/// Create an indicator event with optional progress (0.0–1.0).
pub fn indicator_event_with_progress(
    indicator: IndicatorType,
    message: Option<&str>,
    progress: Option<f64>,
) -> StreamBundle {
    let mut payload = json!({"indicator": indicator});
    if let Some(msg) = message {
        payload["message"] = json!(msg);
    }
    if let Some(p) = progress {
        payload["progress"] = json!(p);
    }
    StreamBundle {
        event_type: EventType::Indicator,
        message_id: String::new(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        description: indicator.as_str().to_owned(),
        payload,
    }
}

/// Create a text delta event for streaming LLM output.
pub fn text_delta_event(
    message_id: &str,
    delta: &str,
    sequence: usize,
    done: bool,
) -> StreamBundle {
    StreamBundle {
        event_type: EventType::TextDelta,
        message_id: message_id.to_owned(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        description: "text_delta".to_owned(),
        payload: json!({
            "delta": delta,
            "sequence": sequence,
            "done": done,
        }),
    }
}

/// Create an error event with code and recoverability classification.
pub fn error_event(message_id: &str, code: &str, message: &str, recoverable: bool) -> StreamBundle {
    StreamBundle {
        event_type: EventType::Error,
        message_id: message_id.to_owned(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        description: "stream_error".to_owned(),
        payload: json!({
            "code": code,
            "message": message,
            "recoverable": recoverable,
        }),
    }
}

/// Create a tool call start event.
pub fn tool_call_start_event(
    message_id: &str,
    tool_name: &str,
    tool_call_id: &str,
) -> StreamBundle {
    StreamBundle {
        event_type: EventType::ToolCallStart,
        message_id: message_id.to_owned(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        description: "tool_call_start".to_owned(),
        payload: json!({
            "tool_name": tool_name,
            "tool_call_id": tool_call_id,
            "status": "start",
        }),
    }
}

/// Create a tool call completion event.
pub fn tool_call_complete_event(
    message_id: &str,
    tool_name: &str,
    tool_call_id: &str,
    success: Option<bool>,
    error: Option<&str>,
    duration_ms: Option<f64>,
) -> StreamBundle {
    StreamBundle {
        event_type: EventType::ToolCallComplete,
        message_id: message_id.to_owned(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        description: "tool_call_complete".to_owned(),
        payload: json!({
            "tool_name": tool_name,
            "tool_call_id": tool_call_id,
            "success": success,
            "error": error,
            "duration_ms": duration_ms,
            "status": "complete",
        }),
    }
}

/// Create an end-of-turn event.
pub fn end_of_turn_event(message_id: &str, sequence: usize) -> StreamBundle {
    text_delta_event(message_id, "", sequence, true)
}

/// Create a heartbeat event.
pub fn heartbeat_event() -> StreamBundle {
    StreamBundle {
        event_type: EventType::Heartbeat,
        message_id: String::new(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        description: "heartbeat".to_owned(),
        payload: json!({}),
    }
}

/// Create a conversation closed event.
pub fn conversation_closed_event(
    conversation_id: &str,
    reason: &str,
    turn_count: u32,
) -> StreamBundle {
    StreamBundle {
        event_type: EventType::ConversationClosed,
        message_id: String::new(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        description: String::new(),
        payload: json!({
            "conversation_id": conversation_id,
            "reason": reason,
            "turn_count": turn_count,
        }),
    }
}
