use serde::{Deserialize, Serialize};

/// Decision result from the decision engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Decision {
    Engage,
    Idle,
    NeedMoreInfo,
}

/// Type of trigger that initiated a proactive action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TriggerType {
    Capture,
    Goal,
    Checkin,
}

/// Context for a triggered proactive action.
#[derive(Debug, Clone)]
pub(crate) struct TriggerContext {
    pub(crate) trigger_type: TriggerType,
    pub(crate) context_text: String,
}
