use std::sync::Arc;
use std::time::Duration;

use github_copilot_sdk::session::Session;
use github_copilot_sdk::types::MessageOptions;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::copilot::error::WorkerError;
use crate::copilot::types::{ChatAttachment, WorkerClass};

#[derive(Debug, Clone)]
pub(crate) struct VisionAnswer {
    pub(crate) text: String,
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

    pub(crate) async fn analyze(
        &self,
        question: &str,
        image: ChatAttachment,
    ) -> Result<VisionAnswer, WorkerError> {
        let _guard = self.submit_lock.lock().await;
        let request_id = Uuid::new_v4();

        let opts = MessageOptions::new(question.to_string())
            .with_wait_timeout(self.turn_timeout)
            .with_attachments(vec![image.into()]);

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
            text,
            output_tokens,
        })
    }

    pub(crate) async fn shutdown(&self) -> Result<(), WorkerError> {
        self.session.destroy().await?;
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "tests panic on precondition failures")]
mod tests {
    use super::*;
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD as BASE64;
    use github_copilot_sdk::types::Attachment;

    #[test]
    fn blob_attachment_round_trips_base64() {
        let raw = b"\x89PNG\r\n\x1a\nfake-png-data";
        let att: Attachment = ChatAttachment::ImageBytes {
            bytes: raw.to_vec(),
            mime_type: "image/png",
        }
        .into();
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
}
