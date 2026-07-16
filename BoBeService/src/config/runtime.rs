//! Background-loop tuning — capture cadence, check-in scheduling,
//! conversation timeouts, decision cooldowns, goal eval interval.
//! Read by various triggers + RuntimeSession; ConfigManager hot-applies
//! some of these (capture, conversation timeouts) live.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct CaptureConfig {
    pub(crate) enabled: bool,
    pub(crate) interval_seconds: u64,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_seconds: 30,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct CheckinConfig {
    pub(crate) enabled: bool,
    pub(crate) times: Vec<String>,
    pub(crate) jitter_minutes: u32,
    pub(crate) interval_minutes: Option<u64>,
}

impl Default for CheckinConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            times: vec!["09:00".into(), "14:00".into(), "19:00".into()],
            jitter_minutes: 5,
            interval_minutes: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct ConversationConfig {
    pub(crate) inactivity_timeout_seconds: u64,
    pub(crate) auto_close_minutes: u64,
    pub(crate) closed_retention_days: u32,
    pub(crate) closed_retention_count: u32,
}

impl Default for ConversationConfig {
    fn default() -> Self {
        Self {
            inactivity_timeout_seconds: 30,
            auto_close_minutes: 10,
            closed_retention_days: 90,
            closed_retention_count: 1_000,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct DecisionConfig {
    pub(crate) cooldown_minutes: i64,
    pub(crate) extended_cooldown_minutes: i64,
    pub(crate) recent_ai_messages_limit: i64,
}

impl Default for DecisionConfig {
    fn default() -> Self {
        Self {
            cooldown_minutes: 3,
            extended_cooldown_minutes: 5,
            recent_ai_messages_limit: 3,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct GoalsConfig {
    pub(crate) check_interval_seconds: f64,
}

impl Default for GoalsConfig {
    fn default() -> Self {
        Self {
            check_interval_seconds: 900.0,
        }
    }
}
