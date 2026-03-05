use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, info, warn};

use crate::error::AppError;
use crate::llm::EmbeddingProvider;

#[derive(Debug, Deserialize)]
struct OllamaEmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

#[derive(Debug, Deserialize)]
struct OpenAiEmbedResponse {
    data: Vec<OpenAiEmbedData>,
}

#[derive(Debug, Deserialize)]
struct OpenAiEmbedData {
    embedding: Vec<f32>,
}

fn validate_embedding(embedding: &[f32], expected_dimension: usize) -> bool {
    if embedding.len() != expected_dimension {
        warn!(
            expected = expected_dimension,
            actual = embedding.len(),
            "embedding.dimension_mismatch"
        );
        return false;
    }
    embedding.iter().all(|v| v.is_finite())
}

pub(crate) struct LocalEmbeddingProvider {
    client: Client,
    base_url: String,
    model: String,
    dimension: usize,
}

impl LocalEmbeddingProvider {
    pub(crate) fn new(client: Client, base_url: &str, model: &str, dimension: usize) -> Self {
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
}

enum OpenAiAuth {
    Bearer(String),
    ApiKey(String),
}

pub(crate) struct OpenAiEmbeddingProvider {
    client: Client,
    endpoint_url: String,
    auth: OpenAiAuth,
    model: Option<String>,
    dimension: usize,
}

impl OpenAiEmbeddingProvider {
    pub(crate) fn openai(client: Client, api_key: &str, model: &str, dimension: usize) -> Self {
        info!(
            backend = "openai",
            model = model,
            dimension = dimension,
            "embedding.provider_created"
        );
        Self {
            client,
            endpoint_url: "https://api.openai.com/v1/embeddings".to_string(),
            auth: OpenAiAuth::Bearer(api_key.to_string()),
            model: Some(model.to_string()),
            dimension,
        }
    }

    pub(crate) fn azure(
        client: Client,
        endpoint: &str,
        api_key: &str,
        deployment: &str,
        dimension: usize,
    ) -> Self {
        let endpoint = endpoint.trim_end_matches('/');
        let endpoint_url = format!(
            "{endpoint}/openai/deployments/{deployment}/embeddings?api-version=2024-02-15-preview"
        );
        info!(
            backend = "azure_openai",
            deployment = deployment,
            dimension = dimension,
            "embedding.provider_created"
        );
        Self {
            client,
            endpoint_url,
            auth: OpenAiAuth::ApiKey(api_key.to_string()),
            model: None,
            dimension,
        }
    }

    async fn request_embeddings(
        &self,
        input: serde_json::Value,
    ) -> Result<Vec<Vec<f32>>, AppError> {
        let mut body = serde_json::json!({
            "input": input,
            "encoding_format": "float",
            "dimensions": self.dimension,
        });
        if let Some(model) = &self.model {
            body["model"] = serde_json::Value::String(model.clone());
        }

        let mut request = self.client.post(&self.endpoint_url).json(&body);
        request = match &self.auth {
            OpenAiAuth::Bearer(api_key) => {
                request.header("Authorization", format!("Bearer {api_key}"))
            }
            OpenAiAuth::ApiKey(api_key) => request.header("api-key", api_key),
        };

        let response = request
            .send()
            .await
            .map_err(|e| AppError::Embedding(format!("Embedding request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(AppError::Embedding(format!(
                "Embedding provider returned {status}: {text}"
            )));
        }

        let resp: OpenAiEmbedResponse = response
            .json()
            .await
            .map_err(|e| AppError::Embedding(format!("Failed to parse embedding response: {e}")))?;

        if resp.data.is_empty() {
            return Err(AppError::Embedding("No embeddings in response".into()));
        }

        let embeddings: Vec<Vec<f32>> = resp.data.into_iter().map(|item| item.embedding).collect();
        for (index, emb) in embeddings.iter().enumerate() {
            if !validate_embedding(emb, self.dimension) {
                return Err(AppError::Embedding(format!(
                    "Invalid embedding at index {index}: expected {} dimensions, got {}",
                    self.dimension,
                    emb.len()
                )));
            }
        }
        Ok(embeddings)
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

        if !validate_embedding(&embedding, self.dimension) {
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
            if !validate_embedding(emb, self.dimension) {
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

#[async_trait]
impl EmbeddingProvider for OpenAiEmbeddingProvider {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, AppError> {
        let mut embeddings = self
            .request_embeddings(serde_json::Value::String(text.to_string()))
            .await?;
        let embedding = embeddings
            .drain(..)
            .next()
            .ok_or_else(|| AppError::Embedding("No embeddings in response".into()))?;
        debug!(dimension = embedding.len(), "embedding.complete");
        Ok(embedding)
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, AppError> {
        if texts.is_empty() {
            return Ok(vec![]);
        }
        let input = serde_json::to_value(texts)
            .map_err(|e| AppError::Embedding(format!("Failed to serialize texts: {e}")))?;
        let embeddings = self.request_embeddings(input).await?;
        debug!(count = embeddings.len(), "embedding.batch_complete");
        Ok(embeddings)
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}
