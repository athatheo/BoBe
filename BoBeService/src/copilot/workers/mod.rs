pub(crate) mod batch;
pub(crate) mod chat;
pub(crate) mod vision;

use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;

use super::error::WorkerError;
use super::types::{ChatDelta, ChatPrompt};

#[async_trait]
pub(crate) trait ChatWorker: Send + Sync {
    async fn send(
        &self,
        prompt: ChatPrompt,
    ) -> Result<Pin<Box<dyn Stream<Item = ChatDelta> + Send>>, WorkerError>;
}
