use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EventType {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[derive(Default)]
pub(crate) enum IndicatorType {
    #[default]
    Idle,
    ScreenCapture,
    Thinking,
    ToolCalling,
    Streaming,
}

impl IndicatorType {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::ScreenCapture => "ScreenCapture",
            Self::Thinking => "Thinking",
            Self::ToolCalling => "ToolCalling",
            Self::Streaming => "Streaming",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StreamBundle {
    #[serde(rename = "type")]
    pub(crate) event_type: EventType,
    pub(crate) message_id: String,
    pub(crate) timestamp: String,
    pub(crate) description: String,
    pub(crate) payload: serde_json::Value,
}
