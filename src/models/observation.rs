use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::types::ObservationSource;

/// Raw captured data with semantic search. Retention: 7 days.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub(crate) struct Observation {
    pub(crate) id: Uuid,
    pub(crate) source: ObservationSource,
    pub(crate) content: String,
    pub(crate) category: String,
    /// JSON-encoded embedding vector.
    pub(crate) embedding: Option<String>,
    /// JSON-encoded flexible metadata.
    pub(crate) metadata: Option<String>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

impl Observation {
    pub(crate) fn new(source: ObservationSource, content: String, category: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            source,
            content,
            category,
            embedding: None,
            metadata: None,
            created_at: now,
            updated_at: now,
        }
    }
}
