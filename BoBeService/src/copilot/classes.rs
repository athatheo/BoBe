//! Worker class catalog. One entry per Copilot CLI session BoBe runs.
//! Adding a new class is one constant here + one accessor on `WorkerRegistry`.
//!
//! Classes share a single SDK `Client` (one Copilot CLI server process for
//! the whole daemon); each accessor lazily spawns its own `Session` on
//! first use. Per-class differentiation is name + turn timeout.

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

/// Stable session class names.
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

    pub(crate) async fn vision(&self) -> Result<Arc<dyn AgentWorker>, AppError> {
        self.get_or_start(WorkerSpec {
            name: names::VISION.into(),
            turn_timeout: Some(Duration::from_mins(5)),
        })
        .await
    }

    pub(crate) async fn chat(&self) -> Result<Arc<dyn AgentWorker>, AppError> {
        self.get_or_start(WorkerSpec {
            name: names::CHAT.into(),
            turn_timeout: Some(Duration::from_mins(2)),
        })
        .await
    }

    pub(crate) async fn consolidate(&self) -> Result<Arc<dyn AgentWorker>, AppError> {
        self.get_or_start(WorkerSpec {
            name: names::CONSOLIDATE.into(),
            turn_timeout: Some(Duration::from_mins(15)),
        })
        .await
    }
}

/// Vision-specific helper: write image bytes to disk, then submit a job
/// referencing the file path. The SDK runtime reads the file, base64-
/// encodes, resizes if needed, and sends it as a vision attachment.
///
/// Future revision: switch to `AttachmentType::Blob` to avoid the temp
/// file entirely once the SDK exposes blob attachments cleanly.
pub(crate) struct VisionRequest {
    pub(crate) image_bytes: Vec<u8>,
    pub(crate) image_ext: &'static str,
    pub(crate) question: String,
}

pub(crate) async fn submit_vision(
    registry: &WorkerRegistry,
    images_dir: &std::path::Path,
    req: VisionRequest,
) -> Result<JobOutput, WorkerError> {
    tokio::fs::create_dir_all(images_dir).await?;

    let job_id = Uuid::new_v4();
    let img_name = format!("{job_id}.{}", req.image_ext);
    let img_path = images_dir.join(&img_name);
    tokio::fs::write(&img_path, &req.image_bytes).await?;

    // The SDK exposes `Attachment::File`/`Blob` via `MessageOptions`, but
    // our `AgentWorker::submit` plumbs `JobInput` only. Until vision lands
    // a proper attachment path through the trait we encode the absolute
    // path in the job text and let Copilot's Read tool fetch it.
    let job = JobInput {
        job_id,
        kind: "vision".into(),
        instructions: format!(
            "Read the image at the absolute path below and answer `question`. \
             Return JSON {{\"answer\":\"<text>\"}}. Be concise.\n\nimage = {}",
            img_path.display()
        ),
        input: json!({ "question": req.question, "image_path": img_path.to_string_lossy() }),
    };

    let worker = registry
        .vision()
        .await
        .map_err(|e| WorkerError::Io(std::io::Error::other(e.to_string())))?;
    worker.submit(job).await
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "tests panic on precondition failures")]
mod tests {
    use super::*;

    #[test]
    fn class_names_are_stable() {
        for name in [
            names::GOALS,
            names::OBSERVE,
            names::VISION,
            names::CHAT,
            names::CONSOLIDATE,
        ] {
            assert!(!name.is_empty());
            assert!(name.len() <= 64);
        }
    }
}
