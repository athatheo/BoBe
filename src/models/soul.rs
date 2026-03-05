use chrono::{DateTime, Utc};

use super::ids::SoulId;

/// Personality document injected into LLM prompts.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub(crate) struct Soul {
    pub(crate) id: SoulId,
    pub(crate) name: String,
    pub(crate) content: String,
    pub(crate) enabled: bool,
    pub(crate) is_default: bool,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

impl Soul {
    pub(crate) fn new(name: String, content: String, is_default: bool) -> Self {
        let now = Utc::now();
        Self {
            id: SoulId::new(),
            name,
            content,
            enabled: true,
            is_default,
            created_at: now,
            updated_at: now,
        }
    }
}
