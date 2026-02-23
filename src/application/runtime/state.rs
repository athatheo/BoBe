use serde::{Deserialize, Serialize};

/// Decision result from the decision engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Engage,
    Ignore,
}

/// Type of trigger that initiated a proactive action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerType {
    Capture,
    Goal,
    Checkin,
    AgentJob,
}

/// Context for a triggered proactive action.
#[derive(Debug, Clone)]
pub struct TriggerContext {
    pub trigger_type: TriggerType,
    pub context_text: String,
}
