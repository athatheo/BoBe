//! Worker implementations. Two flavors of worker, each behind its own
//! trait so consumers can depend on the abstraction:

#![allow(
    dead_code,
    reason = "Phase 6: traits + impls land here; Phase 5 consumer migration wires them up"
)]
//!
//! * **`AgentWorker`** — batch jobs (goals, observe, consolidate, vision).
//!   One `submit(JobInput) -> JobOutput` round-trip per call. Sessions
//!   run in `autopilot` mode so the model auto-loops to completion.
//!
//! * **`ChatWorker`** — live conversation. `send(prompt)` returns a
//!   `Stream<ChatDelta>` that yields token-level deltas, tool events,
//!   and a final `Done`. Sessions run in `interactive` mode and persist
//!   across daemon restarts (with daily rotation — see
//!   [`super::session_store`]).
//!
//! The common machinery — session lifecycle, hooks, handlers, memory
//! injection — lives in the parent module. Workers just consume an
//! already-configured `Session`.

pub(crate) mod batch;
pub(crate) mod chat;
pub(crate) mod vision;

use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;

use super::error::WorkerError;
use super::types::{ChatDelta, ChatPrompt, JobInput, JobOutput, WorkerClass};

/// Batch worker: one job in, one structured result out. Used by the
/// learners (goals, observe) and by the nightly consolidation trigger.
#[async_trait]
pub(crate) trait AgentWorker: Send + Sync {
    async fn submit(&self, job: JobInput) -> Result<JobOutput, WorkerError>;
    fn class(&self) -> WorkerClass;
}

/// Streaming chat worker. Long-lived session per local date. The
/// returned stream yields one `ChatDelta::Done` and then ends.
#[async_trait]
pub(crate) trait ChatWorker: Send + Sync {
    async fn send(
        &self,
        prompt: ChatPrompt,
    ) -> Result<Pin<Box<dyn Stream<Item = ChatDelta> + Send>>, WorkerError>;

    /// Cancel the in-flight turn. No-op if no turn is running.
    async fn abort(&self) -> Result<(), WorkerError>;

    /// Manually compact the chat history. Useful before a high-stakes
    /// turn (e.g. "plan my week") where context budget matters.
    /// **Experimental SDK surface** — may break across SDK versions.
    async fn compact(&self) -> Result<(), WorkerError>;
}
