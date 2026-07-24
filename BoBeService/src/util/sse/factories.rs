use serde_json::json;

use super::types::{EventType, IndicatorType, StreamBundle};

pub(crate) fn indicator_event(indicator: IndicatorType) -> StreamBundle {
    StreamBundle::now(
        EventType::Indicator,
        String::new(),
        indicator.as_str().to_owned(),
        json!({ "indicator": indicator }),
    )
}

pub(crate) fn text_delta_event(
    message_id: &str,
    delta: &str,
    sequence: usize,
    done: bool,
) -> StreamBundle {
    StreamBundle::now(
        EventType::TextDelta,
        message_id.to_owned(),
        "text_delta".to_owned(),
        json!({
            "delta": delta,
            "sequence": sequence,
            "done": done,
        }),
    )
}

pub(crate) fn error_event(
    message_id: &str,
    code: &str,
    message: &str,
    recoverable: bool,
) -> StreamBundle {
    StreamBundle::now(
        EventType::Error,
        message_id.to_owned(),
        "stream_error".to_owned(),
        json!({
            "code": code,
            "message": message,
            "recoverable": recoverable,
        }),
    )
}

/// Distinct payload from chat-stream errors: consumer keys off `trigger`, not `code`.
pub(crate) fn trigger_error_event(trigger: &str, message: &str, recoverable: bool) -> StreamBundle {
    StreamBundle::now(
        EventType::Error,
        uuid::Uuid::new_v4().to_string(),
        format!("{trigger} error"),
        json!({
            "trigger": trigger,
            "message": message,
            "recoverable": recoverable,
        }),
    )
}

pub(crate) fn tool_call_start_event(
    message_id: &str,
    tool_name: &str,
    tool_call_id: &str,
) -> StreamBundle {
    StreamBundle::now(
        EventType::ToolCallStart,
        message_id.to_owned(),
        "tool_call_start".to_owned(),
        json!({
            "tool_name": tool_name,
            "tool_call_id": tool_call_id,
            "status": crate::constants::tool_call_status::START,
        }),
    )
}

pub(crate) fn tool_call_complete_event(
    message_id: &str,
    tool_name: &str,
    tool_call_id: &str,
    success: Option<bool>,
    error: Option<&str>,
    duration_ms: Option<f64>,
) -> StreamBundle {
    StreamBundle::now(
        EventType::ToolCallComplete,
        message_id.to_owned(),
        "tool_call_complete".to_owned(),
        json!({
            "tool_name": tool_name,
            "tool_call_id": tool_call_id,
            "success": success,
            "error": error,
            "duration_ms": duration_ms,
            "status": crate::constants::tool_call_status::COMPLETE,
        }),
    )
}

pub(crate) fn end_of_turn_event(
    message_id: &str,
    sequence: usize,
    full_response: &str,
) -> StreamBundle {
    text_delta_event(message_id, full_response, sequence, true)
}

pub(crate) fn heartbeat_event() -> StreamBundle {
    StreamBundle::now(
        EventType::Heartbeat,
        String::new(),
        "heartbeat".to_owned(),
        json!({}),
    )
}

pub(crate) fn conversation_closed_event(
    conversation_id: &str,
    reason: &str,
    turn_count: u32,
) -> StreamBundle {
    StreamBundle::now(
        EventType::ConversationClosed,
        String::new(),
        String::new(),
        json!({
            "conversation_id": conversation_id,
            "reason": reason,
            "turn_count": turn_count,
        }),
    )
}

pub(crate) fn conversation_changed_event(conversation_id: &str) -> StreamBundle {
    StreamBundle::now(
        EventType::ConversationChanged,
        String::new(),
        "conversation_changed".to_owned(),
        json!({ "conversation_id": conversation_id }),
    )
}
