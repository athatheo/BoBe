use serde::{Deserialize, Serialize};

/// SSE event types matching the Python implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    Indicator,
    TextDelta,
    ToolCall,
    ToolCallStart,
    ToolCallComplete,
    Error,
    Heartbeat,
    EndOfTurn,
    ConversationClosed,
}

/// Indicator states for the runtime session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[derive(Default)]
pub enum IndicatorType {
    #[default]
    Idle,
    ScreenCapture,
    Thinking,
    ToolCalling,
    Streaming,
}


/// Wire format for SSE events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamBundle {
    #[serde(rename = "type")]
    pub event_type: EventType,
    pub message_id: String,
    pub timestamp: String,
    pub description: String,
    pub payload: serde_json::Value,
}
