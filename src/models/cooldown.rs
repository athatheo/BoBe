use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

/// Result of a cooldown check when cooldown is active.
#[derive(Debug, Clone)]
pub struct CooldownInfo {
    pub remaining: Duration,
    pub cooldown_minutes: i64,
    /// `"user_response"` or `"ai_engagement"`
    pub cooldown_type: String,
}

/// Tracks cooldown state for proactive engagement.
///
/// Single-row table — enforced by application logic.
/// Survives server restarts (ADR-0003).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct Cooldown {
    pub id: Uuid,
    pub last_engagement: Option<DateTime<Utc>>,
    pub last_user_response: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Cooldown {
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            last_engagement: None,
            last_user_response: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Check if cooldown period is active.
    /// Returns `Some(CooldownInfo)` if in cooldown, `None` if ready to engage.
    pub fn check_cooldown(&self, base_minutes: i64, extended_minutes: i64) -> Option<CooldownInfo> {
        let now = Utc::now();

        if let Some(last_response) = self.last_user_response {
            let extended = Duration::minutes(extended_minutes);
            let elapsed = now - last_response;
            if elapsed < extended {
                return Some(CooldownInfo {
                    remaining: extended - elapsed,
                    cooldown_minutes: extended_minutes,
                    cooldown_type: "user_response".to_owned(),
                });
            }
        }

        if let Some(last_eng) = self.last_engagement {
            let base = Duration::minutes(base_minutes);
            let elapsed = now - last_eng;
            if elapsed < base {
                return Some(CooldownInfo {
                    remaining: base - elapsed,
                    cooldown_minutes: base_minutes,
                    cooldown_type: "ai_engagement".to_owned(),
                });
            }
        }

        None
    }
}

impl Default for Cooldown {
    fn default() -> Self {
        Self::new()
    }
}
