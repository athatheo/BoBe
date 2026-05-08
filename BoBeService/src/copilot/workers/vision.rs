//! `VisionWorker` — image-input batch worker. Doesn't implement
//! `AgentWorker` because its signature is unique (image + question
//! instead of generic JSON input). Consumers depend on this concrete
//! type directly.

#![allow(
    dead_code,
    reason = "Phase 6: vision worker complete; Phase 5 wires the screen capture path"
)]
//!
//! Implementation: builds a `MessageOptions` with an `Attachment::Blob`
//! (in-memory base64 — no temp files) or `Attachment::File`, sends in
//! `autopilot` mode, blocks on `send_and_wait`, returns the assistant's
//! reply as plain text. Vision answers don't need to be JSON; the
//! caller usually wants natural-language descriptions.

use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use github_copilot_sdk::session::Session;
use github_copilot_sdk::types::{Attachment, MessageOptions};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::copilot::error::WorkerError;
use crate::copilot::types::{ChatAttachment, WorkerClass};

#[derive(Debug, Clone)]
pub(crate) struct VisionAnswer {
    pub(crate) request_id: Uuid,
    /// Natural-language answer from the model.
    pub(crate) text: String,
    /// Output token count if reported by the SDK; useful for cost rollups.
    pub(crate) output_tokens: Option<u64>,
}

pub(crate) struct VisionWorker {
    session: Arc<Session>,
    submit_lock: Mutex<()>,
    turn_timeout: Duration,
}

impl VisionWorker {
    pub(crate) fn new(session: Arc<Session>) -> Arc<Self> {
        Arc::new(Self {
            session,
            submit_lock: Mutex::new(()),
            turn_timeout: WorkerClass::Vision.turn_timeout(),
        })
    }

    /// Send an image + question, get a natural-language answer.
    pub(crate) async fn analyze(
        &self,
        question: &str,
        image: ChatAttachment,
    ) -> Result<VisionAnswer, WorkerError> {
        let _guard = self.submit_lock.lock().await;
        let request_id = Uuid::new_v4();

        let attachment = to_sdk_attachment(image)?;
        // Session mode (autopilot) set on the SessionConfig in registry,
        // not on MessageOptions (which is delivery-mode only).
        let opts = MessageOptions::new(question.to_string())
            .with_wait_timeout(self.turn_timeout)
            .with_attachments(vec![attachment]);

        let event = self
            .session
            .send_and_wait(opts)
            .await?
            .ok_or(WorkerError::NoAssistantMessage(request_id))?;

        let text = event
            .data
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let output_tokens = event
            .data
            .get("outputTokens")
            .and_then(serde_json::Value::as_u64);

        Ok(VisionAnswer {
            request_id,
            text,
            output_tokens,
        })
    }

    pub(crate) async fn shutdown(&self) -> Result<(), WorkerError> {
        self.session.destroy().await?;
        Ok(())
    }
}

fn to_sdk_attachment(att: ChatAttachment) -> Result<Attachment, WorkerError> {
    match att {
        ChatAttachment::ImageBytes { bytes, mime_type } => Ok(Attachment::Blob {
            data: BASE64.encode(&bytes),
            mime_type: mime_type.to_string(),
            display_name: None,
        }),
        ChatAttachment::File { path } => {
            if !path.is_absolute() {
                return Err(WorkerError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("vision file attachment must be absolute: {}", path.display()),
                )));
            }
            Ok(Attachment::File {
                path,
                display_name: None,
                line_range: None,
            })
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "tests panic on precondition failures")]
mod tests {
    use super::*;

    #[test]
    fn blob_attachment_round_trips_base64() {
        let raw = b"\x89PNG\r\n\x1a\nfake-png-data";
        let att = to_sdk_attachment(ChatAttachment::ImageBytes {
            bytes: raw.to_vec(),
            mime_type: "image/png",
        })
        .unwrap();
        match att {
            Attachment::Blob {
                data, mime_type, ..
            } => {
                assert_eq!(mime_type, "image/png");
                assert_eq!(BASE64.decode(data).unwrap(), raw);
            }
            other => panic!("expected Blob, got {other:?}"),
        }
    }

    #[test]
    fn file_attachment_requires_absolute_path() {
        let err = to_sdk_attachment(ChatAttachment::File {
            path: std::path::PathBuf::from("relative.png"),
        })
        .unwrap_err();
        match err {
            WorkerError::Io(io) => assert_eq!(io.kind(), std::io::ErrorKind::InvalidInput),
            other => panic!("expected Io error, got {other}"),
        }
    }
}
