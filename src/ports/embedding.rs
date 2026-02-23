use async_trait::async_trait;

use crate::error::AppError;

/// Protocol for text embedding providers.
///
/// Embeddings are used for semantic search and similarity matching.
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Embed text into a vector.
    async fn embed(&self, text: &str) -> Result<Vec<f32>, AppError>;

    /// Embed multiple texts efficiently (batched).
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, AppError>;

    /// Get the dimension of embeddings produced by this provider.
    fn dimension(&self) -> usize;
}
