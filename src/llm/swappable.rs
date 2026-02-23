//! A transparent LlmProvider wrapper backed by ArcSwap.
//!
//! When ConfigManager rebuilds the LLM provider (e.g. after an API key or model
//! change), all consumers holding a reference to `SwappableLlmProvider`
//! automatically see the new provider on their next call -- no update callbacks
//! needed.

use std::pin::Pin;
use std::sync::Arc;

use arc_swap::ArcSwap;
use async_trait::async_trait;
use futures::Stream;

use crate::error::AppError;
use crate::llm::types::{AiMessage, AiResponse, ResponseFormat, StreamChunk, ToolDefinition};
use crate::llm::LlmProvider;

/// A [`LlmProvider`] that delegates every call to whatever provider is
/// currently stored in the inner [`ArcSwap`].
pub struct SwappableLlmProvider {
    inner: Arc<ArcSwap<Arc<dyn LlmProvider>>>,
}

impl SwappableLlmProvider {
    /// Create a new swappable wrapper around `initial`.
    ///
    /// Returns the wrapper **and** the `ArcSwap` handle that
    /// [`ConfigManager`] uses to hot-swap the underlying provider.
    pub fn new(initial: Arc<dyn LlmProvider>) -> (Self, Arc<ArcSwap<Arc<dyn LlmProvider>>>) {
        let swappable = Arc::new(ArcSwap::from_pointee(initial));
        let provider = Self {
            inner: swappable.clone(),
        };
        (provider, swappable)
    }
}

#[async_trait]
impl LlmProvider for SwappableLlmProvider {
    async fn complete(
        &self,
        messages: &[AiMessage],
        tools: Option<&[ToolDefinition]>,
        response_format: Option<&ResponseFormat>,
        temperature: f32,
        max_tokens: u32,
    ) -> Result<AiResponse, AppError> {
        let provider = self.inner.load();
        provider
            .complete(messages, tools, response_format, temperature, max_tokens)
            .await
    }

    fn stream(
        &self,
        messages: Vec<AiMessage>,
        tools: Option<Vec<ToolDefinition>>,
        response_format: Option<ResponseFormat>,
        temperature: f32,
        max_tokens: u32,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamChunk, AppError>> + Send + '_>> {
        // Load the full Arc so the provider outlives the stream.
        let provider = self.inner.load_full();
        Box::pin(async_stream::stream! {
            let stream = provider.stream(messages, tools, response_format, temperature, max_tokens);
            tokio::pin!(stream);
            while let Some(item) = futures::StreamExt::next(&mut stream).await {
                yield item;
            }
        })
    }

    async fn health_check(&self) -> bool {
        self.inner.load().health_check().await
    }

    fn supports_vision(&self) -> bool {
        self.inner.load().supports_vision()
    }

    fn supports_tools(&self) -> bool {
        self.inner.load().supports_tools()
    }
}
