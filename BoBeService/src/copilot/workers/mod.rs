//! Worker implementations. Two flavors:
//!
//! * `AgentWorker` — batch jobs (goals, observe, consolidate, vision +
//!   decide). One `submit(JobInput) -> JobOutput` round-trip per call.
//!   Sessions run in autopilot mode so the model auto-loops to completion.
//! * `ChatWorker` — live streaming conversation. `send(prompt)` returns
//!   a `Stream<ChatDelta>` that yields token-level deltas, tool events,
//!   and a terminal `Done`/`Error`. Sessions run in interactive mode
//!   and persist across daemon restarts (daily rotation — see
//!   [`super::session_store`]).

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
