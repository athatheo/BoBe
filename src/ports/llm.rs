use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

use super::llm_types::{
    AiMessage, AiResponse, ResponseFormat, StreamChunk, ToolDefinition,
};
use crate::error::AppError;

/// Protocol for LLM providers.
///
/// All implementations (Ollama, llama.cpp, OpenAI, Azure) conform to this trait.
/// The application layer only knows about this trait — never concrete implementations.
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
    fn supports_tools(&self) -> bool;
}
