use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Markdown document describing the user, injected into LLM prompts.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub(crate) struct UserProfile {
    pub(crate) id: Uuid,
    pub(crate) name: String,
    pub(crate) content: String,
    pub(crate) enabled: bool,
    pub(crate) is_default: bool,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

impl UserProfile {
    pub(crate) fn new(name: String, content: String, is_default: bool) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            content,
            enabled: true,
            is_default,
            created_at: now,
            updated_at: now,
        }
    }
}
