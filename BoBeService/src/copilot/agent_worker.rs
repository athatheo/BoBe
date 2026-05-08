//! `AgentWorker` trait — the abstraction that learners and services consume.
//! Decouples them from `github_copilot_sdk::Session` plumbing: callers see
//! "submit a job, await a result." Implementing types live in `worker.rs`.

#![allow(
    dead_code,
    reason = "Phase 2: trait declared here; consumers (learners, services) cut over in Phase 5"
)]

use async_trait::async_trait;

use super::worker::{JobInput, JobOutput, WorkerError};

#[async_trait]
pub(crate) trait AgentWorker: Send + Sync {
    /// Submit one job, block until the SDK signals `session.idle`.
    /// Implementors must serialize concurrent submits — only one job in
    /// flight per session at a time.
    async fn submit(&self, job: JobInput) -> Result<JobOutput, WorkerError>;

    /// Stable name for tracing / registry lookup.
    fn name(&self) -> &str;
}
