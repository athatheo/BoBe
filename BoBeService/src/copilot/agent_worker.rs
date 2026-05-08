//! `AgentWorker` trait — the abstraction that learners and services
//! consume. Decouples them from `CopilotWorker`'s tmux/PTY concerns:
//! callers see only "submit a job, await a result."

#![allow(
    dead_code,
    reason = "Phase 2: trait + impl land here; consumers (learners, services) cut over in Phase 5"
)]

use async_trait::async_trait;

use super::worker::{CopilotWorker, JobInput, JobOutput, WorkerError};

#[async_trait]
pub(crate) trait AgentWorker: Send + Sync {
    /// Submit one job, block until the worker emits an outbox file.
    /// Implementors must serialize concurrent submits (one in-flight
    /// per session) — `CopilotWorker` does this with `submit_lock`.
    async fn submit(&self, job: JobInput) -> Result<JobOutput, WorkerError>;

    /// Stable name for tracing / registry lookup.
    fn name(&self) -> &str;
}

#[async_trait]
impl AgentWorker for CopilotWorker {
    async fn submit(&self, job: JobInput) -> Result<JobOutput, WorkerError> {
        Self::submit(self, job).await
    }

    fn name(&self) -> &str {
        Self::name(self)
    }
}
