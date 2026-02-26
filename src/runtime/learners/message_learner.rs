//! Message learner — embeds and stores user messages as observations.
//!
//! No LLM distillation needed; simply generates an embedding and persists.

use std::sync::Arc;

use tracing::{debug, info};

use crate::db::ObservationRepository;
use crate::llm::EmbeddingProvider;
use crate::models::observation::Observation;
use crate::models::types::ObservationSource;
use crate::runtime::learners::types::{
    LearnerError, LearnerObservation, LearnerObservationSource, LearnerResult,
};

pub struct MessageLearner {
    embedding: Arc<dyn EmbeddingProvider>,
    observation_repo: Arc<dyn ObservationRepository>,
}

impl MessageLearner {
    pub fn new(
        embedding: Arc<dyn EmbeddingProvider>,
        observation_repo: Arc<dyn ObservationRepository>,
    ) -> Self {
        Self {
            embedding,
            observation_repo,
        }
    }

    /// Embed and store a user message observation.
    pub async fn learn(
        &self,
        observation: &LearnerObservation,
    ) -> Result<LearnerResult, LearnerError> {
        if observation.source != LearnerObservationSource::Message {
            return Err(LearnerError::WrongSource {
                expected: "message".into(),
                got: observation.source.as_str().into(),
            });
        }

        let message_text = observation
            .text
            .as_ref()
            .ok_or_else(|| LearnerError::MissingData("text content required".into()))?;

        info!(
            message_length = message_text.len(),
            message_preview = %if message_text.len() > 80 {
                format!("{}...", &message_text[..80])
            } else {
                message_text.clone()
            },
            "message_learner.start"
        );

        // 1. Generate embedding
        let embedding_vec = self
            .embedding
            .embed(message_text)
            .await
            .map_err(|e| LearnerError::Embedding(e.to_string()))?;

        debug!(vector_dim = embedding_vec.len(), "message_learner.embedded");

        // 2. Create and store observation
        let mut obs = Observation::new(
            ObservationSource::Screen, // Using Screen as placeholder; source field is String
            message_text.clone(),
            "conversation".into(),
        );
        obs.source = ObservationSource::UserMessage;
        obs.embedding = Some(
            serde_json::to_string(&embedding_vec)
                .map_err(|e| LearnerError::Storage(e.to_string()))?,
        );
        let summary = Self::create_summary(message_text);
        let meta = serde_json::json!({ "summary": summary });
        obs.metadata = Some(meta.to_string());

        let stored = self.observation_repo.save(&obs).await?;

        info!(
            item_id = %stored.id,
            message_length = message_text.len(),
            "message_learner.complete"
        );

        Ok(LearnerResult::Stored {
            observation_id: stored.id,
        })
    }

    fn create_summary(text: &str) -> String {
        let max_len = 150;
        let cleaned: String = text.split_whitespace().collect::<Vec<_>>().join(" ");

        if cleaned.len() <= max_len {
            return cleaned;
        }

        let truncated = &cleaned[..max_len];
        if let Some(last_space) = truncated.rfind(' ')
            && last_space > 50
        {
            return format!("{}...", &truncated[..last_space]);
        }
        format!("{truncated}...")
    }
}
