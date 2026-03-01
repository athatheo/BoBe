use serde::{Deserialize, Serialize};

use crate::models::observation::Observation;

/// Decision result from the decision engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Engage,
    Idle,
    Ignore,
    NeedMoreInfo,
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
    /// Full observation (with embedding) for capture triggers.
    pub observation: Option<Observation>,
    /// Goal model for goal triggers (set by GoalTrigger, reserved for future use by DecisionEngine).
    #[allow(dead_code)]
    pub goal: Option<crate::models::goal::Goal>,
}
