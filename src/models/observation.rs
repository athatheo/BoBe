use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::types::ObservationSource;

/// Raw captured data with semantic search capability.
///
/// Source abstraction: ScreenCapture → source="screen", etc.
/// Retention: 7 days (daily pruning job).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct Observation {
    pub id: Uuid,
    pub source: ObservationSource,
    pub content: String,
    pub category: String,
    /// JSON-encoded embedding vector.
    pub embedding: Option<String>,
    /// JSON-encoded flexible metadata.
    pub metadata: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Observation {
    pub fn new(source: ObservationSource, content: String, category: String) -> Self {
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
