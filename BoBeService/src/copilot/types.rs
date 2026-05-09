//! Shared types for the Copilot worker system. Mirrors the
//! `llm::types` pattern: data shapes only, no behavior. Workers, hooks,
//! and the registry all import from here.

#![allow(
    dead_code,
    reason = "Phase 6 catalog: variants/types declared here; Phase 5 consumer migration uses them"
)]

use std::time::Duration;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Stable identifier for a worker class. Used as the session label, the
/// directory name under `~/.bobe/workers/`, and the skill directory under
/// `~/.bobe/skills/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum WorkerClass {
    /// Goal extraction from conversation history.
    Goals,
    /// Observation extraction from screen captures or text.
    Observe,
    /// Vision answering — image attachment + question.
    Vision,
    /// Live conversation with the user. Long-lived session, daily rotation.
    Chat,
    /// Nightly memory.md compaction.
    Consolidate,
}

impl WorkerClass {
    /// All classes in declaration order — useful for shutdown loops.
    pub(crate) const fn all() -> &'static [WorkerClass] {
        &[
            WorkerClass::Goals,
            WorkerClass::Observe,
            WorkerClass::Vision,
            WorkerClass::Chat,
            WorkerClass::Consolidate,
        ]
    }

    /// Stable ASCII name. Also the tmux/session identifier and the
    /// directory component under `~/.bobe/workers/`.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            WorkerClass::Goals => "goals",
            WorkerClass::Observe => "observe",
            WorkerClass::Vision => "vision",
            WorkerClass::Chat => "chat",
            WorkerClass::Consolidate => "consolidate",
        }
    }

    /// Per-turn deadline. Vision and consolidation need longer than chat.
    /// Observe and Chat happen to share a 2-minute timeout, but the
    /// values are independent — keep both arms explicit for future
    /// tuning.
    #[allow(
        clippy::match_same_arms,
        reason = "per-class tuning; identical-by-coincidence values"
    )]
    pub(crate) const fn turn_timeout(self) -> Duration {
        match self {
            WorkerClass::Goals => Duration::from_mins(3),
            WorkerClass::Observe => Duration::from_mins(2),
            WorkerClass::Vision => Duration::from_mins(5),
            WorkerClass::Chat => Duration::from_mins(2),
            WorkerClass::Consolidate => Duration::from_mins(15),
        }
    }

    /// Mode the SDK should use for this class. Autopilot for headless
    /// batch jobs (model auto-loops to `task_complete`); Interactive
    /// for live conversation where each turn returns a single reply.
    pub(crate) const fn mode(self) -> &'static str {
        match self {
            WorkerClass::Chat => "interactive",
            _ => "autopilot",
        }
    }
}

/// Batch worker job request. `instructions` is freeform per-job context;
/// the worker's stable identity comes from its `SKILL.md`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct JobInput {
    pub(crate) job_id: Uuid,
    pub(crate) kind: String,
    pub(crate) instructions: String,
    #[serde(default)]
    pub(crate) input: serde_json::Value,
}

/// Result of a batch job. Workers return JSON; we parse `output` from the
/// assistant message and surface raw text + parse error if it fails.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct JobOutput {
    pub(crate) job_id: Uuid,
    #[serde(default)]
    pub(crate) output: serde_json::Value,
    #[serde(default)]
    pub(crate) text: String,
    #[serde(default)]
    pub(crate) error: Option<String>,
}

/// User-side input to a chat turn. Attachments cover image / file inputs.
#[derive(Debug, Clone)]
pub(crate) struct ChatPrompt {
    pub(crate) text: String,
    pub(crate) attachments: Vec<ChatAttachment>,
}

impl ChatPrompt {
    pub(crate) fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            attachments: Vec::new(),
        }
    }
}

/// Inputs we forward to the Copilot SDK as `Attachment` variants. Subset
/// of the SDK's full `Attachment` enum — only what BoBe currently emits.
#[derive(Debug, Clone)]
pub(crate) enum ChatAttachment {
    /// In-memory image bytes. Lifted into the SDK as `Attachment::Blob`
    /// (no disk I/O — base64 inline). Preferred for screenshots.
    ImageBytes {
        bytes: Vec<u8>,
        mime_type: &'static str,
    },
    /// Existing file on disk. Lifted as `Attachment::File`. Use for
    /// images / docs the daemon already wrote.
    File { path: std::path::PathBuf },
}

/// Streaming chunks from a chat turn. Adapter from the SDK's session
/// event stream into a BoBe-shaped vocabulary the SwiftUI overlay can
/// render.
#[derive(Debug, Clone)]
pub(crate) enum ChatDelta {
    /// Token-level streaming text — append to the current message bubble.
    MessageDelta(String),
    /// Final, authoritative message body (deltas may have been re-streamed
    /// or out-of-order; this is the source of truth).
    MessageComplete {
        content: String,
        output_tokens: Option<u64>,
    },
    /// A tool the assistant is invoking (UI shows "🔍 searching files...").
    /// `id` correlates with the matching `ToolComplete` for SSE display.
    ToolStart {
        id: String,
        name: String,
        args: serde_json::Value,
    },
    /// Tool finished. `success` reflects exit, not whether the model was
    /// happy with the result. `id` matches the corresponding `ToolStart`.
    ToolComplete {
        id: String,
        name: String,
        success: bool,
    },
    /// Recoverable error mid-stream. Stream may continue.
    Error(String),
    /// Turn complete. Stream ends after this.
    Done,
}

/// Snapshot of cost / quota usage for a worker session over its lifetime.
/// Aggregated by `usage::UsageMeter` from `assistant.usage` events.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct UsageSnapshot {
    pub(crate) input_tokens: u64,
    pub(crate) output_tokens: u64,
    pub(crate) cache_read_tokens: u64,
    pub(crate) cache_write_tokens: u64,
    /// Sum of `cost` fields across all `assistant.usage` events. Cost is
    /// the model multiplier — multiply by quota price to get $ amount.
    pub(crate) cost_units: f64,
    /// Number of API calls (one `assistant.usage` event = one call).
    pub(crate) api_calls: u64,
}
