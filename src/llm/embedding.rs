use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, info, warn};

use crate::error::AppError;
use crate::llm::EmbeddingProvider;

/// Response from Ollama's `/api/embed` endpoint.
#[derive(Debug, Deserialize)]
struct OllamaEmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

/// Local embedding provider that calls Ollama's embedding endpoint.
///
/// Uses the `/api/embed` API with a configurable model (default: nomic-embed-text).
pub struct LocalEmbeddingProvider {
    client: Client,
    base_url: String,
    model: String,
    dimension: usize,
}

impl LocalEmbeddingProvider {
    pub fn new(client: Client, base_url: &str, model: &str, dimension: usize) -> Self {
        info!(
            model = model,
            dimension = dimension,
            "embedding.provider_created"
        );
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_owned(),
            model: model.to_owned(),
            dimension,
        }
    }

    /// Validate an embedding has the expected dimension and all finite values.
    fn validate_embedding(&self, embedding: &[f32]) -> bool {
        if embedding.len() != self.dimension {
            warn!(
                expected = self.dimension,
                actual = embedding.len(),
                "embedding.dimension_mismatch"
            );
            return false;
        }
        embedding.iter().all(|v| v.is_finite())
    }
}

#[async_trait]
impl EmbeddingProvider for LocalEmbeddingProvider {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, AppError> {
        let url = format!("{}/api/embed", self.base_url);
        let body = serde_json::json!({
            "model": self.model,
            "input": text,
            "keep_alive": "45s",
        });

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Embedding(format!("Ollama embed request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(AppError::Embedding(format!(
                "Ollama embed returned {status}: {text}"
            )));
        }

        let resp: OllamaEmbedResponse = response
            .json()
            .await
            .map_err(|e| AppError::Embedding(format!("Failed to parse embed response: {e}")))?;

        let embedding = resp
            .embeddings
            .into_iter()
            .next()
            .ok_or_else(|| AppError::Embedding("No embeddings in response".into()))?;

        if !self.validate_embedding(&embedding) {
            return Err(AppError::Embedding(format!(
                "Invalid embedding: expected {} dimensions, got {}",
                self.dimension,
                embedding.len()
            )));
        }

        debug!(dimension = embedding.len(), "embedding.complete");
        Ok(embedding)
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, AppError> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        let url = format!("{}/api/embed", self.base_url);
        let body = serde_json::json!({
            "model": self.model,
            "input": texts,
            "keep_alive": "45s",
        });

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Embedding(format!("Ollama batch embed failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(AppError::Embedding(format!(
                "Ollama batch embed returned {status}: {text}"
            )));
        }

        let resp: OllamaEmbedResponse = response
            .json()
            .await
            .map_err(|e| AppError::Embedding(format!("Failed to parse batch response: {e}")))?;

        for (i, emb) in resp.embeddings.iter().enumerate() {
            if !self.validate_embedding(emb) {
                return Err(AppError::Embedding(format!(
                    "Invalid embedding at index {i}: expected {} dimensions, got {}",
                    self.dimension,
                    emb.len()
                )));
            }
        }

        debug!(count = resp.embeddings.len(), "embedding.batch_complete");
        Ok(resp.embeddings)
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}

/// Compute cosine similarity between two embedding vectors.
#[allow(dead_code)]
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}
