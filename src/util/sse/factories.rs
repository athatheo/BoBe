use serde_json::json;

use super::types::{EventType, IndicatorType, StreamBundle};

/// Create an indicator event showing the daemon's current activity.
pub fn indicator_event(indicator: IndicatorType, message: Option<&str>) -> StreamBundle {
    let mut payload = json!({"indicator": indicator});
    if let Some(msg) = message {
        payload["message"] = json!(msg);
    }
    StreamBundle {
        event_type: EventType::Indicator,
        message_id: String::new(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        description: String::new(),
        payload,
    }
}

/// Create a text delta event for streaming LLM output.
#[allow(dead_code)]
pub fn text_delta_event(message_id: &str, delta: &str, sequence: u64, done: bool) -> StreamBundle {
    StreamBundle {
        event_type: EventType::TextDelta,
        message_id: message_id.to_owned(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        description: String::new(),
        payload: json!({
            "delta": delta,
            "sequence": sequence,
            "done": done,
        }),
    }
}

/// Create an error event.
#[allow(dead_code)]
pub fn error_event(code: &str, message: &str, recoverable: bool) -> StreamBundle {
    StreamBundle {
        event_type: EventType::Error,
        message_id: String::new(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        description: String::new(),
        payload: json!({
            "code": code,
            "message": message,
            "recoverable": recoverable,
        }),
    }
}

/// Create a heartbeat event for keep-alive.
#[allow(dead_code)]
pub fn heartbeat_event() -> StreamBundle {
    StreamBundle {
        event_type: EventType::Heartbeat,
        message_id: String::new(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        description: String::new(),
        payload: json!({}),
    }
}

/// Create a tool call start event.
#[allow(dead_code)]
pub fn tool_call_start_event(
    message_id: &str,
    tool_name: &str,
    tool_call_id: &str,
) -> StreamBundle {
    StreamBundle {
        event_type: EventType::ToolCall,
        message_id: message_id.to_owned(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        description: String::new(),
        payload: json!({
            "status": "start",
            "tool_name": tool_name,
            "tool_call_id": tool_call_id,
        }),
    }
}

/// Create a tool call complete event.
#[allow(dead_code)]
pub fn tool_call_complete_event(
    message_id: &str,
    tool_name: &str,
    tool_call_id: &str,
    success: bool,
    error: Option<&str>,
    duration_ms: Option<f64>,
) -> StreamBundle {
    let mut payload = json!({
        "status": "complete",
        "tool_name": tool_name,
        "tool_call_id": tool_call_id,
        "success": success,
    });
    if let Some(err) = error {
        payload["error"] = json!(err);
    }
    if let Some(ms) = duration_ms {
        payload["duration_ms"] = json!(ms);
    }
    StreamBundle {
        event_type: EventType::ToolCall,
        message_id: message_id.to_owned(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        description: String::new(),
        payload,
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

/// Create an end-of-turn event.
#[allow(dead_code)]
pub fn end_of_turn_event(message_id: &str) -> StreamBundle {
    StreamBundle {
        event_type: EventType::EndOfTurn,
        message_id: message_id.to_owned(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        description: String::new(),
        payload: json!({}),
    }
}

/// Create an action request event for user interaction (e.g. goal worker ask_user).
#[allow(dead_code)]
pub fn action_request_event(
    action: &str,
    prompt: &str,
    request_id: &str,
    timeout_ms: u64,
    options: Option<&[String]>,
) -> StreamBundle {
    let mut payload = json!({
        "action": action,
        "prompt": prompt,
        "request_id": request_id,
        "timeout_ms": timeout_ms,
    });
    if let Some(opts) = options {
        payload["options"] = json!(opts);
    }
    StreamBundle {
        event_type: EventType::ActionRequest,
        message_id: String::new(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        description: String::new(),
        payload,
    }
}
