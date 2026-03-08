pub(crate) mod circuit_breaker;
pub(crate) mod embedding;
pub(crate) mod factory;
pub(crate) mod ollama_manager;
pub(crate) mod providers;
pub(crate) mod shared;
pub(crate) mod swappable;
pub(crate) mod types;

// ─── Trait definitions ──────────────────────────────────────────────────────

use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

use crate::error::AppError;
use types::{AiMessage, AiResponse, ResponseFormat, StreamChunk, ToolDefinition};

#[async_trait]
pub(crate) trait LlmProvider: Send + Sync {
    async fn complete(
        &self,
        messages: &[AiMessage],
        tools: Option<&[ToolDefinition]>,
        response_format: Option<&ResponseFormat>,
        temperature: f32,
        max_tokens: u32,
    ) -> Result<AiResponse, AppError>;

    fn stream(
        &self,
        messages: Vec<AiMessage>,
        tools: Option<Vec<ToolDefinition>>,
        response_format: Option<ResponseFormat>,
        temperature: f32,
        max_tokens: u32,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamChunk, AppError>> + Send + '_>>;

    async fn health_check(&self) -> bool;
    fn supports_vision(&self) -> bool;

    #[allow(dead_code)]
    fn supports_tools(&self) -> bool;
}

#[async_trait]
pub(crate) trait EmbeddingProvider: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, AppError>;

    #[allow(dead_code)]
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, AppError>;

    #[allow(dead_code)]
    fn dimension(&self) -> usize;
}
