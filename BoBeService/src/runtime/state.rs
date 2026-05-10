use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Decision {
    Engage,
    Idle,
    NeedMoreInfo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TriggerType {
    Capture,
    Goal,
    Checkin,
}

#[derive(Debug, Clone)]
pub(crate) struct TriggerContext {
    pub(crate) trigger_type: TriggerType,
    pub(crate) context_text: String,
}
