//! Worker error type. Internal to the `copilot` module; the rest of the
//! daemon converts these into `crate::error::AppError` at boundaries.

#![allow(
    dead_code,
    reason = "Phase 6: variants surface as Phase 5 consumer migration wires error paths"
)]

use std::time::Duration;

use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub(crate) enum WorkerError {
    /// SDK-level error — protocol, RPC, transport, etc. Check
    /// `is_transport_failure` on the source to know if the underlying
    /// `Client` should be discarded.
    #[error("copilot SDK: {0}")]
    Sdk(#[from] github_copilot_sdk::Error),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),

    /// Turn exceeded its deadline. Retry is safe — Copilot's own
    /// `WaiterGuard` clears the in-flight slot on cancellation.
    #[error("turn timed out after {}s", .0.as_secs())]
    Timeout(Duration),

    /// Worker returned no parseable JSON when one was required (batch
    /// jobs only — chat doesn't expect JSON).
    #[error("worker returned no parseable JSON output for job {0}")]
    NoJsonOutput(Uuid),

    /// `send_and_wait` resolved without an `assistant.message` event.
    /// Usually means the session errored mid-turn.
    #[error("worker returned no assistant.message event for job {0}")]
    NoAssistantMessage(Uuid),

    /// The SDK reported `session.error` during a turn.
    #[error("session error ({error_type}): {message}")]
    SessionError {
        error_type: String,
        message: String,
    },
}
