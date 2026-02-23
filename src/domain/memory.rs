use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::types::{MemorySource, MemoryType};

/// Distilled knowledge with semantic search capability.
///
/// Retention policies:
/// - `short_term`: 30 days
/// - `long_term`: 90 days
/// - `explicit`: Forever (user said "remember this")
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct Memory {
    pub id: Uuid,
    pub content: String,
    pub memory_type: String,
    pub enabled: bool,
    pub category: String,
    pub source: String,
    /// JSON-encoded embedding vector.
    pub embedding: Option<String>,
    pub source_observation_id: Option<Uuid>,
    pub source_conversation_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Memory {
    pub fn new(
        content: String,
        memory_type: MemoryType,
        source: MemorySource,
        category: String,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            content,
            memory_type: memory_type.as_str().to_owned(),
            enabled: true,
            category,
            source: source.as_str().to_owned(),
            embedding: None,
            source_observation_id: None,
            source_conversation_id: None,
            created_at: now,
            updated_at: now,
        }
    }
}
