use chrono::{DateTime, Utc};
use uuid::Uuid;

/// User profile document describing the user.
///
/// A markdown document used by the AI to understand who the user is
/// and provide personalized assistance.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct UserProfile {
    pub id: Uuid,
    pub name: String,
    pub content: String,
    pub enabled: bool,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl UserProfile {
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
