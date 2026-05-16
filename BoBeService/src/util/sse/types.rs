use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EventType {
    Indicator,
    TextDelta,
    ToolCallStart,
    ToolCallComplete,
    Error,
    Heartbeat,
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
    Streaming,
}

impl IndicatorType {
    /// Wire-format string. Must match the `#[serde(rename_all =
    /// "SCREAMING_SNAKE_CASE")]` attr on the enum so the manual JSON
    /// callers (`runtime/session.rs::get_status`, SSE
    /// `StreamBundle.description`) agree with what serde emits and what
    /// `BoBeMacUI/BoBe/Models/AppState.swift::IndicatorType` decodes.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "IDLE",
            Self::ScreenCapture => "SCREEN_CAPTURE",
            Self::Thinking => "THINKING",
            Self::Streaming => "STREAMING",
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
