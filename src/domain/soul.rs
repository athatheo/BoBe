use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Personality document injected into LLM prompts.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct Soul {
    pub id: Uuid,
    pub name: String,
    pub content: String,
    pub enabled: bool,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Soul {
    pub fn new(name: String, content: String, is_default: bool) -> Self {
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
