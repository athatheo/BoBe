use serde::{Deserialize, Serialize};

use crate::models::observation::Observation;

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
    AgentJob,
}

/// Context for a triggered proactive action.
#[derive(Debug, Clone)]
pub(crate) struct TriggerContext {
    pub(crate) trigger_type: TriggerType,
    pub(crate) context_text: String,
    /// Full observation (with embedding) for capture triggers.
    pub(crate) observation: Option<Observation>,
    /// Goal model for goal triggers (set by GoalTrigger, reserved for future use by DecisionEngine).
    #[allow(dead_code)]
    pub(crate) goal: Option<crate::models::goal::Goal>,
}
