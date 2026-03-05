//! Hot-swappable LLM provider proxy (Rust equivalent of .NET's IOptionsMonitor pattern).
//!
//! In Rust, `Arc<dyn LlmProvider>` is an immutable shared reference — once cloned
//! into 10+ consumers at bootstrap, there's no way to swap what's behind it.
//! `ArcSwap` solves this with lock-free atomic swaps, but consumers expect
//! `Arc<dyn LlmProvider>`, not `Arc<ArcSwap<...>>`.
//!
//! `SwappableLlmProvider` bridges this: it implements `LlmProvider` and delegates
//! every call to whatever provider is currently in the `ArcSwap`. Consumers see
//! a normal `Arc<dyn LlmProvider>` and are unaware of swapping. When
//! `ConfigManager` rebuilds the provider (API key change, model switch, backend
//! swap), all consumers automatically use the new provider on their next call.
//!
//! Uses `ArcSwapOption` so the provider can start as `None` when LLM config is
//! incomplete (fresh install). The HTTP server boots immediately; once onboarding
//! completes, `ConfigManager.rebuild_llm()` swaps in a real provider.
//!
//! The atomic load per call is essentially free (one `Acquire` fence, nanoseconds).

use std::pin::Pin;
use std::sync::Arc;

use arc_swap::ArcSwapOption;
use async_trait::async_trait;
use futures::Stream;

use crate::error::AppError;
use crate::llm::EmbeddingProvider;
use crate::llm::LlmProvider;
use crate::llm::types::{AiMessage, AiResponse, ResponseFormat, StreamChunk, ToolDefinition};

/// A [`LlmProvider`] that delegates every call to whatever provider is
/// currently stored in the inner [`ArcSwapOption`].
pub(crate) struct SwappableLlmProvider {
    inner: Arc<ArcSwapOption<Arc<dyn LlmProvider>>>,
}

impl SwappableLlmProvider {
    /// Create a new swappable wrapper around `initial`.
    ///
    /// Returns the wrapper **and** the `ArcSwapOption` handle that
    /// [`ConfigManager`] uses to hot-swap the underlying provider.
    pub(crate) fn new(
        initial: Arc<dyn LlmProvider>,
    ) -> (Self, Arc<ArcSwapOption<Arc<dyn LlmProvider>>>) {
        let swappable = Arc::new(ArcSwapOption::from(Some(Arc::new(initial))));
        let provider = Self {
            inner: swappable.clone(),
        };
        (provider, swappable)
    }

    /// Create an empty swappable wrapper (no provider yet).
    ///
    /// Used on fresh install when LLM config is incomplete. The HTTP server
    /// starts immediately; all LLM calls return `LlmUnavailable` until
    /// onboarding completes and a real provider is swapped in.
    pub(crate) fn new_empty() -> (Self, Arc<ArcSwapOption<Arc<dyn LlmProvider>>>) {
        let swappable = Arc::new(ArcSwapOption::empty());
        let provider = Self {
            inner: swappable.clone(),
        };
        (provider, swappable)
    }
}

fn no_provider_error() -> AppError {
    AppError::LlmUnavailable("LLM not configured — complete setup".into())
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
        // load_full() returns an owned Arc, releasing the epoch guard immediately.
        // load() would pin the epoch for the entire LLM call (10-30s+).
        let provider = self.inner.load_full().ok_or_else(no_provider_error)?;
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
        let provider = self.inner.load_full();
        Box::pin(async_stream::stream! {
            let Some(ref provider) = provider else {
                yield Err(no_provider_error());
                return;
            };
            let stream = provider.stream(messages, tools, response_format, temperature, max_tokens);
            tokio::pin!(stream);
            while let Some(item) = futures::StreamExt::next(&mut stream).await {
                yield item;
            }
        })
    }

    async fn health_check(&self) -> bool {
        match self.inner.load_full() {
            Some(provider) => provider.health_check().await,
            None => false,
        }
    }

    fn supports_vision(&self) -> bool {
        let guard = self.inner.load();
        guard.as_ref().is_some_and(|p| p.supports_vision())
    }

    fn supports_tools(&self) -> bool {
        let guard = self.inner.load();
        guard.as_ref().is_some_and(|p| p.supports_tools())
    }
}

/// A [`EmbeddingProvider`] that delegates every call to the provider currently
/// stored in an inner [`ArcSwapOption`].
pub(crate) struct SwappableEmbeddingProvider {
    inner: Arc<ArcSwapOption<Arc<dyn EmbeddingProvider>>>,
}

impl SwappableEmbeddingProvider {
    pub(crate) fn new(
        initial: Arc<dyn EmbeddingProvider>,
    ) -> (Self, Arc<ArcSwapOption<Arc<dyn EmbeddingProvider>>>) {
        let swappable = Arc::new(ArcSwapOption::from(Some(Arc::new(initial))));
        let provider = Self {
            inner: swappable.clone(),
        };
        (provider, swappable)
    }

    /// Create an empty swappable wrapper (no embedding provider yet).
    ///
    /// Used on fresh install when embedding config is incomplete. All embed
    /// calls return `LlmUnavailable` until onboarding completes.
    pub(crate) fn new_empty() -> (Self, Arc<ArcSwapOption<Arc<dyn EmbeddingProvider>>>) {
        let swappable = Arc::new(ArcSwapOption::empty());
        let provider = Self {
            inner: swappable.clone(),
        };
        (provider, swappable)
    }
}

fn no_embedding_error() -> AppError {
    AppError::LlmUnavailable("Embedding not configured — complete setup".into())
}

#[async_trait]
impl EmbeddingProvider for SwappableEmbeddingProvider {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, AppError> {
        let provider = self.inner.load_full().ok_or_else(no_embedding_error)?;
        provider.embed(text).await
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, AppError> {
        let provider = self.inner.load_full().ok_or_else(no_embedding_error)?;
        provider.embed_batch(texts).await
    }

    fn dimension(&self) -> usize {
        let guard = self.inner.load();
        guard.as_ref().map_or(0, |p| p.dimension())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_llm_returns_unavailable() {
        let (provider, _handle) = SwappableLlmProvider::new_empty();
        let result = provider.complete(&[], None, None, 0.7, 100).await;
        assert!(matches!(result, Err(AppError::LlmUnavailable(_))));
        assert!(!provider.health_check().await);
        assert!(!provider.supports_vision());
        assert!(!provider.supports_tools());
    }

    #[tokio::test]
    async fn empty_embedding_returns_unavailable() {
        let (provider, _handle) = SwappableEmbeddingProvider::new_empty();
        let result = provider.embed("test").await;
        assert!(matches!(result, Err(AppError::LlmUnavailable(_))));
        let batch_result = provider.embed_batch(&["test".to_owned()]).await;
        assert!(matches!(batch_result, Err(AppError::LlmUnavailable(_))));
        assert_eq!(provider.dimension(), 0);
    }
}
