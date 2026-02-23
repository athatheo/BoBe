use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::domain::observation::Observation;

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
    /// Goal model for goal triggers.
    pub goal: Option<crate::domain::goal::Goal>,
}

/// Configuration for the orchestrator (immutable per cycle).
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    pub capture_enabled: bool,
    pub capture_interval_seconds: u64,
    pub decision_cooldown_minutes: i64,
    pub decision_extended_cooldown_minutes: i64,
    pub min_context_for_decision: usize,
    pub semantic_search_limit: i64,
    pub recent_ai_messages_limit: i64,
    pub max_response_tokens: u32,
    pub response_temperature: f32,
    pub checkin_enabled: bool,
    pub checkin_times: Vec<String>,
    pub checkin_jitter_minutes: u32,
    pub goal_check_interval_seconds: f64,
    pub conversation_inactivity_timeout_seconds: u64,
    pub conversation_auto_close_minutes: u64,
    pub conversation_summary_enabled: bool,
    pub tools_enabled: bool,
}

impl OrchestratorConfig {
    pub fn from_config(config: &Config) -> Self {
        let times: Vec<String> = config
            .checkin_times
            .split(',')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect();

        Self {
            capture_enabled: config.capture_enabled,
            capture_interval_seconds: config.capture_interval_seconds,
            decision_cooldown_minutes: 3,
            decision_extended_cooldown_minutes: 5,
            min_context_for_decision: 2,
            semantic_search_limit: 10,
            recent_ai_messages_limit: 3,
            max_response_tokens: 500,
            response_temperature: 0.7,
            checkin_enabled: config.checkin_enabled,
            checkin_times: times,
            checkin_jitter_minutes: config.checkin_jitter_minutes,
            goal_check_interval_seconds: config.goal_check_interval_seconds,
            conversation_inactivity_timeout_seconds: config.conversation_inactivity_timeout_seconds,
            conversation_auto_close_minutes: config.conversation_auto_close_minutes,
            conversation_summary_enabled: config.conversation_summary_enabled,
            tools_enabled: config.tools_enabled,
        }
    }
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            capture_enabled: true,
            capture_interval_seconds: 240,
            decision_cooldown_minutes: 3,
            decision_extended_cooldown_minutes: 5,
            min_context_for_decision: 2,
            semantic_search_limit: 10,
            recent_ai_messages_limit: 3,
            max_response_tokens: 500,
            response_temperature: 0.7,
            checkin_enabled: true,
            checkin_times: vec!["09:00".into(), "14:00".into(), "19:00".into()],
            checkin_jitter_minutes: 5,
            goal_check_interval_seconds: 900.0,
            conversation_inactivity_timeout_seconds: 30,
            conversation_auto_close_minutes: 10,
            conversation_summary_enabled: true,
            tools_enabled: true,
        }
    }
}
