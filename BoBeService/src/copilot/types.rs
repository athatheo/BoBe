use std::time::Duration;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum WorkerClass {
    Goals,
    Vision,
    Chat,
    Consolidate,
    Decide,
}

impl WorkerClass {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            WorkerClass::Goals => "goals",
            WorkerClass::Vision => "vision",
            WorkerClass::Chat => "chat",
            WorkerClass::Consolidate => "consolidate",
            WorkerClass::Decide => "decide",
        }
    }

    pub(crate) const fn turn_timeout(self) -> Duration {
        match self {
            WorkerClass::Goals => Duration::from_mins(3),
            WorkerClass::Vision => Duration::from_mins(5),
            WorkerClass::Chat => Duration::from_mins(2),
            WorkerClass::Consolidate => Duration::from_mins(15),
            // Decide is hot-path: every screen capture waits on it.
            WorkerClass::Decide => Duration::from_secs(45),
        }
    }

    pub(crate) const fn mode(self) -> &'static str {
        match self {
            WorkerClass::Chat => "interactive",
            _ => "autopilot",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct JobInput {
    pub(crate) job_id: Uuid,
    pub(crate) kind: String,
    pub(crate) instructions: String,
    #[serde(default)]
    pub(crate) input: serde_json::Value,
}

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

#[derive(Debug, Clone)]
pub(crate) enum ChatAttachment {
    /// Lifted to `Attachment::Blob` (base64 inline, no disk I/O).
    ImageBytes {
        bytes: Vec<u8>,
        mime_type: &'static str,
    },
}

#[derive(Debug, Clone)]
pub(crate) enum ChatDelta {
    MessageDelta(String),
    /// Authoritative final body; deltas may have been re-streamed out-of-order.
    MessageComplete {
        content: String,
        output_tokens: Option<u64>,
    },
    ToolStart { id: String, name: String },
    /// `success` reflects exit, not model satisfaction.
    ToolComplete {
        id: String,
        name: String,
        success: bool,
    },
    Error(String),
    Done,
}

