use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_stream::stream;
use futures::Stream;
use github_copilot_sdk::session::Session;
use github_copilot_sdk::subscription::RecvError;
use github_copilot_sdk::types::{DeliveryMode, MessageOptions, SessionEvent, SetModelOptions};
use tokio::sync::Mutex;

use crate::copilot::error::WorkerError;
use crate::copilot::types::{ChatDelta, ChatPrompt};

pub(crate) struct CopilotChatWorker {
    session: Arc<Session>,
    submit_lock: Arc<Mutex<()>>,
    /// Model name the session was created with. Needed so we can call
    /// `Session::set_model` to tune `reasoning_effort` per-turn (low for
    /// voice, medium for text). `None` means the SDK picked its default —
    /// in that case we skip set_model and the default effort applies.
    model: Option<String>,
    /// Last effort we sent via set_model; skip the redundant RPC if the
    /// new send wants the same effort. None = never called set_model.
    last_effort: Arc<Mutex<Option<&'static str>>>,
}

/// Without this, a dropped stream leaks an in-flight turn — wasted tokens + residual events.
struct AbortGuard {
    session: Arc<Session>,
    completed: Arc<AtomicBool>,
}

impl Drop for AbortGuard {
    fn drop(&mut self) {
        if self.completed.load(Ordering::Acquire) {
            return;
        }
        let session = Arc::clone(&self.session);
        tokio::spawn(async move {
            if let Err(e) = session.abort().await {
                tracing::warn!(err = %e, "chat stream dropped; abort failed");
            } else {
                tracing::debug!("chat stream dropped; aborted in-flight turn");
            }
        });
    }
}

impl CopilotChatWorker {
    pub(crate) fn new(session: Arc<Session>, model: Option<String>) -> Arc<Self> {
        Arc::new(Self {
            session,
            submit_lock: Arc::new(Mutex::new(())),
            model,
            last_effort: Arc::new(Mutex::new(None)),
        })
    }

    pub(crate) async fn shutdown(&self) -> Result<(), WorkerError> {
        // disconnect (not destroy) preserves on-disk state for resume within the same date.
        self.session.disconnect().await?;
        Ok(())
    }

    pub(crate) fn session(&self) -> Arc<Session> {
        Arc::clone(&self.session)
    }
}

impl CopilotChatWorker {
    pub(crate) async fn send(
        &self,
        prompt: ChatPrompt,
    ) -> Result<Pin<Box<dyn Stream<Item = ChatDelta> + Send>>, WorkerError> {
        let session = Arc::clone(&self.session);
        let lock = Arc::clone(&self.submit_lock);
        let model = self.model.clone();
        let last_effort = Arc::clone(&self.last_effort);
        let voice_mode = prompt.voice_mode;

        let abort_guard = AbortGuard {
            session: Arc::clone(&session),
            completed: Arc::new(AtomicBool::new(false)),
        };
        let completed = Arc::clone(&abort_guard.completed);

        let s = stream! {
            let _abort_on_drop = abort_guard;
            let _guard = lock.lock_owned().await;

            let mut events = session.subscribe();

            // Tune reasoning_effort per turn — low for voice (TTFT-critical),
            // medium for text (default). Skipped when model is None (SDK
            // default applies) or when the effort hasn't changed since the
            // last send (avoids a ~30ms RPC per turn for back-to-back same-
            // mode sends).
            if let Some(model_name) = model.as_deref() {
                let want_effort: &'static str = if voice_mode { "low" } else { "medium" };
                // Read-only check first; only mark the slot updated AFTER
                // set_model succeeds. Caching the effort BEFORE the await
                // strands the worker on the wrong effort if set_model
                // returns Err — next same-effort send would skip the RPC
                // and inherit the SDK's actual (stale) reasoning level.
                // submit_lock above serializes calls into this scope so
                // the lock-twice pattern is safe.
                let needs_update = last_effort.lock().await.as_deref() != Some(want_effort);
                if needs_update {
                    let opts = SetModelOptions::default().with_reasoning_effort(want_effort);
                    match session.set_model(model_name, Some(opts)).await {
                        Ok(()) => {
                            *last_effort.lock().await = Some(want_effort);
                        }
                        Err(e) => {
                            tracing::warn!(
                                err = %e,
                                voice_mode,
                                effort = want_effort,
                                "chat.set_model_failed_continuing"
                            );
                        }
                    }
                }
            }

            let opts = match build_message_options(prompt) {
                Ok(o) => o,
                Err(e) => {
                    completed.store(true, Ordering::Release);
                    yield ChatDelta::Error(format!("build message options: {e}"));
                    return;
                }
            };

            if let Err(e) = session.send(opts).await {
                completed.store(true, Ordering::Release);
                yield ChatDelta::Error(format!("send failed: {e}"));
                return;
            }

            loop {
                match events.recv().await {
                    Ok(event) => {
                        if let Some(delta) = event_to_delta(&event) {
                            let stop = matches!(delta, ChatDelta::Done | ChatDelta::Error(_));
                            yield delta;
                            if stop {
                                completed.store(true, Ordering::Release);
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        if let RecvError::Lagged(skipped) = &e {
                            tracing::warn!(
                                skipped = skipped.skipped(),
                                "chat subscription lagged — events dropped"
                            );
                            continue;
                        }
                        completed.store(true, Ordering::Release);
                        yield ChatDelta::Error(format!("event stream: {e}"));
                        return;
                    }
                }
            }
        };

        Ok(Box::pin(s))
    }
}

fn build_message_options(prompt: ChatPrompt) -> Result<MessageOptions, std::io::Error> {
    let mut attachments = Vec::with_capacity(prompt.attachments.len());
    for att in prompt.attachments {
        attachments.push(att.into());
    }

    let mut opts = MessageOptions::new(prompt.text);
    if !attachments.is_empty() {
        opts = opts.with_attachments(attachments);
    }
    if prompt.voice_mode {
        opts = opts.with_mode(DeliveryMode::Immediate);
    }
    Ok(opts)
}

/// Typed SDK event payloads. Each variant matches one `event.event_type`
/// arm in `event_to_delta`. `serde(default)` rescues *missing* fields
/// (empty string / false) — type-mismatched fields or a non-object
/// `event.data` still fail decode and drop the event via `.ok()?`. In
/// practice the SDK always sends `Value::Object` for these event types,
/// and `response_streamer` already filters empty deltas downstream.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct MessageDeltaPayload {
    #[serde(default)]
    delta_content: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct MessagePayload {
    #[serde(default)]
    content: String,
    output_tokens: Option<u64>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolStartPayload {
    #[serde(default)]
    tool_call_id: String,
    #[serde(default)]
    tool_name: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolCompletePayload {
    #[serde(default)]
    tool_call_id: String,
    #[serde(default)]
    tool_name: String,
    #[serde(default)]
    success: bool,
}

#[derive(serde::Deserialize)]
struct SessionErrorPayload {
    message: Option<String>,
}

fn event_to_delta(event: &SessionEvent) -> Option<ChatDelta> {
    let payload = || event.data.clone();
    match event.event_type.as_str() {
        "assistant.message_delta" => {
            let p: MessageDeltaPayload = serde_json::from_value(payload()).ok()?;
            // Empty delta = nothing to emit. Differs slightly from the old
            // `.and_then(.as_str).map(...)` which would emit Some("") for a
            // present-but-empty field, but `response_streamer` filters empty
            // text anyway, so consumers see the same behavior.
            if p.delta_content.is_empty() {
                None
            } else {
                Some(ChatDelta::MessageDelta(p.delta_content))
            }
        }
        "assistant.message" => {
            let p: MessagePayload = serde_json::from_value(payload()).ok()?;
            Some(ChatDelta::MessageComplete {
                content: p.content,
                output_tokens: p.output_tokens,
            })
        }
        "tool.execution_start" => {
            let p: ToolStartPayload = serde_json::from_value(payload()).ok()?;
            Some(ChatDelta::ToolStart {
                id: p.tool_call_id,
                name: p.tool_name,
            })
        }
        "tool.execution_complete" => {
            let p: ToolCompletePayload = serde_json::from_value(payload()).ok()?;
            Some(ChatDelta::ToolComplete {
                id: p.tool_call_id,
                name: p.tool_name,
                success: p.success,
            })
        }
        "session.idle" => Some(ChatDelta::Done),
        "session.error" => {
            let p: SessionErrorPayload = serde_json::from_value(payload()).ok()?;
            Some(ChatDelta::Error(
                p.message.unwrap_or_else(|| "session error".into()),
            ))
        }
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "tests panic on precondition failures")]
mod tests {
    use super::*;
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD as BASE64;
    use github_copilot_sdk::types::Attachment;
    use serde_json::json;

    use crate::copilot::types::ChatAttachment;

    fn ev(event_type: &str, data: serde_json::Value) -> SessionEvent {
        SessionEvent {
            id: "id".into(),
            timestamp: "2026-05-08T00:00:00Z".into(),
            parent_id: None,
            ephemeral: None,
            agent_id: None,
            debug_cli_received_at_ms: None,
            debug_ws_forwarded_at_ms: None,
            event_type: event_type.into(),
            data,
        }
    }

    #[test]
    fn message_delta_maps_to_chatdelta() {
        let e = ev(
            "assistant.message_delta",
            json!({ "messageId": "m1", "deltaContent": "Hello" }),
        );
        match event_to_delta(&e) {
            Some(ChatDelta::MessageDelta(s)) => assert_eq!(s, "Hello"),
            other => panic!("expected MessageDelta, got {other:?}"),
        }
    }

    #[test]
    fn message_complete_extracts_output_tokens() {
        let e = ev(
            "assistant.message",
            json!({ "messageId": "m1", "content": "done", "outputTokens": 42 }),
        );
        match event_to_delta(&e) {
            Some(ChatDelta::MessageComplete {
                content,
                output_tokens,
            }) => {
                assert_eq!(content, "done");
                assert_eq!(output_tokens, Some(42));
            }
            other => panic!("expected MessageComplete, got {other:?}"),
        }
    }

    #[test]
    fn tool_events_round_trip() {
        let start = ev(
            "tool.execution_start",
            json!({
                "toolCallId": "tc-1",
                "toolName": "bash",
                "arguments": { "cmd": "ls" }
            }),
        );
        match event_to_delta(&start) {
            Some(ChatDelta::ToolStart { id, name }) => {
                assert_eq!(id, "tc-1");
                assert_eq!(name, "bash");
            }
            other => panic!("expected ToolStart, got {other:?}"),
        }
        let complete = ev(
            "tool.execution_complete",
            json!({ "toolCallId": "tc-1", "toolName": "bash", "success": true }),
        );
        match event_to_delta(&complete) {
            Some(ChatDelta::ToolComplete { id, name, success }) => {
                assert_eq!(id, "tc-1");
                assert_eq!(name, "bash");
                assert!(success);
            }
            other => panic!("expected ToolComplete, got {other:?}"),
        }
    }

    #[test]
    fn idle_signals_done_and_error_signals_error() {
        let idle = ev("session.idle", json!({}));
        assert!(matches!(event_to_delta(&idle), Some(ChatDelta::Done)));

        let error = ev(
            "session.error",
            json!({ "errorType": "rate_limit", "message": "slow down" }),
        );
        match event_to_delta(&error) {
            Some(ChatDelta::Error(s)) => assert_eq!(s, "slow down"),
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn ignored_event_returns_none() {
        let e = ev("assistant.turn_start", json!({ "turnId": "1" }));
        assert!(event_to_delta(&e).is_none());
    }

    #[test]
    fn build_options_with_blob_attachment() {
        let prompt = ChatPrompt {
            text: "what's in this?".into(),
            attachments: vec![ChatAttachment::ImageBytes {
                bytes: b"PNG-bytes".to_vec(),
                mime_type: "image/png",
            }],
            ..ChatPrompt::default()
        };
        let opts = build_message_options(prompt).unwrap();
        let attachments = opts.attachments.unwrap();
        assert_eq!(attachments.len(), 1);
        match &attachments[0] {
            Attachment::Blob {
                data, mime_type, ..
            } => {
                assert_eq!(mime_type, "image/png");
                assert_eq!(BASE64.decode(data).unwrap(), b"PNG-bytes");
            }
            other => panic!("expected Blob, got {other:?}"),
        }
    }

    #[test]
    fn voice_prompt_sets_immediate_delivery() {
        let prompt = ChatPrompt::voice("hello there");
        let opts = build_message_options(prompt).unwrap();
        assert_eq!(opts.mode, Some(DeliveryMode::Immediate));
    }

    #[test]
    fn text_prompt_leaves_default_delivery() {
        let prompt = ChatPrompt::text("hello there");
        let opts = build_message_options(prompt).unwrap();
        assert_eq!(opts.mode, None);
    }
}
