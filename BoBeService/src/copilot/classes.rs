//! Worker class catalog. One entry per Copilot CLI session BoBe runs.
//! Adding a new class is one constant here + one accessor on `Registry`.
//!
//! Each class shares the same Copilot CLI binary + flags + memory.md
//! symlink; the only differences are:
//!   - name (also the tmux session name and `~/.bobe/workers/<name>/`)
//!   - turn timeout (vision/consolidate need longer than chat/goals)
//!
//! Consumers get an `Arc<dyn AgentWorker>` from the registry and submit
//! `JobInput` whose `kind` field selects the prompt shape on the worker
//! side. The worker's standing instruction (in `.github/copilot-instructions.md`,
//! a symlink to memory.md) plus the per-job `instructions` describe the
//! job format; the worker writes a `JobOutput` to outbox/<id>.json.

#![allow(
    dead_code,
    reason = "Phase 3: catalog + accessors; consumers cut over in Phase 5"
)]

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use uuid::Uuid;

use crate::error::AppError;

use super::agent_worker::AgentWorker;
use super::registry::{WorkerRegistry, WorkerSpec};
use super::worker::{JobInput, JobOutput, WorkerError};

/// Stable session names. Kept as constants so the same string lands in
/// tmux, the worker dir, and any tracing fields.
pub(crate) mod names {
    pub(crate) const GOALS: &str = "bobe-goals";
    pub(crate) const OBSERVE: &str = "bobe-observe";
    pub(crate) const VISION: &str = "bobe-vision";
    pub(crate) const CHAT: &str = "bobe-chat";
    pub(crate) const CONSOLIDATE: &str = "bobe-consolidate";
}

impl WorkerRegistry {
    pub(crate) async fn goals(&self) -> Result<Arc<dyn AgentWorker>, AppError> {
        self.get_or_start(WorkerSpec {
            name: names::GOALS.into(),
            turn_timeout: Some(Duration::from_mins(3)),
        })
        .await
    }

    pub(crate) async fn observe(&self) -> Result<Arc<dyn AgentWorker>, AppError> {
        self.get_or_start(WorkerSpec {
            name: names::OBSERVE.into(),
            turn_timeout: Some(Duration::from_mins(2)),
        })
        .await
    }

    /// Vision worker has the longest "normal" turn — image inspection
    /// can be slower than text-only completions.
    pub(crate) async fn vision(&self) -> Result<Arc<dyn AgentWorker>, AppError> {
        self.get_or_start(WorkerSpec {
            name: names::VISION.into(),
            turn_timeout: Some(Duration::from_mins(5)),
        })
        .await
    }

    /// Chat worker — interactive turns, shorter timeout so the UI doesn't
    /// hang on a stuck Copilot session.
    pub(crate) async fn chat(&self) -> Result<Arc<dyn AgentWorker>, AppError> {
        self.get_or_start(WorkerSpec {
            name: names::CHAT.into(),
            turn_timeout: Some(Duration::from_mins(2)),
        })
        .await
    }

    /// Consolidation rewrites the entire memory.md, can take a while —
    /// 15 minutes covers heavy nights without spurious timeouts.
    pub(crate) async fn consolidate(&self) -> Result<Arc<dyn AgentWorker>, AppError> {
        self.get_or_start(WorkerSpec {
            name: names::CONSOLIDATE.into(),
            turn_timeout: Some(Duration::from_mins(15)),
        })
        .await
    }
}

/// Vision-specific helper: write image bytes into the worker's
/// `images/<uuid>.<ext>`, then submit a job pointing at that file.
///
/// The worker dir is `~/.bobe/workers/bobe-vision/` so Copilot sees the
/// image as `images/<id>.jpg` from its cwd. Copilot's built-in Read tool
/// handles JPEG/PNG natively.
pub(crate) struct VisionRequest {
    pub(crate) image_bytes: Vec<u8>,
    pub(crate) image_ext: &'static str,
    pub(crate) question: String,
}

pub(crate) async fn submit_vision(
    registry: &WorkerRegistry,
    data_dir: &std::path::Path,
    req: VisionRequest,
) -> Result<JobOutput, WorkerError> {
    let worker = registry
        .vision()
        .await
        .map_err(|e| WorkerError::Io(std::io::Error::other(e.to_string())))?;

    let images_dir = data_dir.join("workers").join(names::VISION).join("images");
    tokio::fs::create_dir_all(&images_dir).await?;

    let job_id = Uuid::new_v4();
    let img_name = format!("{job_id}.{}", req.image_ext);
    let img_path = images_dir.join(&img_name);
    tokio::fs::write(&img_path, &req.image_bytes).await?;

    let job = JobInput {
        job_id,
        kind: "vision".into(),
        instructions: format!(
            "Read images/{img_name} and answer the user's question. \
             Return JSON {{\"answer\": \"<text>\"}}. Be concise."
        ),
        input: json!({ "question": req.question, "image_path": format!("images/{img_name}") }),
    };

    worker.submit(job).await
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "tests panic on precondition failures")]
mod tests {
    use super::*;

    #[test]
    fn class_names_are_stable() {
        // Each constant is a tmux session name + dir component. The Mux
        // validator enforces [A-Za-z0-9._-]{1,64} — verify we're inside it.
        for name in [
            names::GOALS,
            names::OBSERVE,
            names::VISION,
            names::CHAT,
            names::CONSOLIDATE,
        ] {
            assert!(!name.is_empty());
            assert!(name.len() <= 64);
            assert!(
                name.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.'),
                "invalid char in {name}"
            );
        }
    }
}
