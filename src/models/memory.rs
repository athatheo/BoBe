use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::types::{MemorySource, MemoryType};

/// Distilled knowledge with semantic search.
///
/// Retention: short_term=30d, long_term=90d, explicit=forever.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub(crate) struct Memory {
    pub(crate) id: Uuid,
    pub(crate) content: String,
    pub(crate) memory_type: MemoryType,
    pub(crate) enabled: bool,
    pub(crate) category: String,
    pub(crate) source: MemorySource,
    /// JSON-encoded embedding vector.
    pub(crate) embedding: Option<String>,
    pub(crate) source_observation_id: Option<Uuid>,
    pub(crate) source_conversation_id: Option<Uuid>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

impl Memory {
    pub(crate) fn new(
        content: String,
        memory_type: MemoryType,
        source: MemorySource,
        category: String,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            content,
            memory_type,
            enabled: true,
            category,
            source,
            embedding: None,
            source_observation_id: None,
            source_conversation_id: None,
            created_at: now,
            updated_at: now,
        }
    }
}
