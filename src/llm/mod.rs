pub mod circuit_breaker;
pub mod embedding;
pub mod factory;
pub mod ollama_manager;
pub mod providers;
pub mod shared;
pub mod swappable;
pub mod types;

// ─── Trait definitions ──────────────────────────────────────────────────────

use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

use crate::error::AppError;
use types::{AiMessage, AiResponse, ResponseFormat, StreamChunk, ToolDefinition};

/// Protocol for LLM providers.
///
/// All implementations (Ollama, llama.cpp, OpenAI, Azure) conform to this trait.
/// The application layer only knows about this trait -- never concrete implementations.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Non-streaming completion.
    async fn complete(
        &self,
        messages: &[AiMessage],
        tools: Option<&[ToolDefinition]>,
        response_format: Option<&ResponseFormat>,
        temperature: f32,
        max_tokens: u32,
    ) -> Result<AiResponse, AppError>;

    /// Streaming completion. Returns a boxed stream of chunks.
    fn stream(
        &self,
        messages: Vec<AiMessage>,
        tools: Option<Vec<ToolDefinition>>,
        response_format: Option<ResponseFormat>,
        temperature: f32,
        max_tokens: u32,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamChunk, AppError>> + Send + '_>>;

    /// Check if provider is available.
    async fn health_check(&self) -> bool;

    /// Whether this provider supports image inputs.
    fn supports_vision(&self) -> bool;

    /// Whether this provider supports tool/function calling.
    #[allow(dead_code)]
    fn supports_tools(&self) -> bool;
}

/// Protocol for text embedding providers.
///
/// Embeddings are used for semantic search and similarity matching.
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Embed text into a vector.
    async fn embed(&self, text: &str) -> Result<Vec<f32>, AppError>;

    /// Embed multiple texts efficiently (batched).
    #[allow(dead_code)]
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, AppError>;

    /// Get the dimension of embeddings produced by this provider.
    #[allow(dead_code)]
    fn dimension(&self) -> usize;
}
