//! `ChatWorker` — streaming chat with the user. Long-lived session per
//! local date (rotation handled by [`super::super::session_store`]). The
//! `send` method returns a `Stream<ChatDelta>` that yields token-level
//! deltas, tool-execution events, and a terminal `Done`.

#![allow(
    dead_code,
    reason = "Phase 6: chat worker complete; Phase 5 wires the SwiftUI overlay path"
)]
//!
//! Session lifecycle (per turn):
//!
//! 1. Acquire `submit_lock` so only one turn is in flight per worker.
//! 2. `session.subscribe()` to start receiving events from now on.
//! 3. `session.send(opts)` to enqueue the prompt (returns message ID
//!    immediately; events arrive via the subscription).
//! 4. Yield `ChatDelta`s from the event stream until `session.idle`
//!    (success) or `session.error` (failure).
//! 5. Drop the lock when the stream is consumed or dropped.

use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_stream::stream;
use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use futures::Stream;
use github_copilot_sdk::session::Session;
use github_copilot_sdk::subscription::RecvError;
use github_copilot_sdk::types::{Attachment, MessageOptions, SessionEvent};
use tokio::sync::Mutex;

use crate::copilot::error::WorkerError;
use crate::copilot::types::{ChatAttachment, ChatDelta, ChatPrompt, WorkerClass};

use super::ChatWorker;

pub(crate) struct CopilotChatWorker {
    session: Arc<Session>,
    submit_lock: Arc<Mutex<()>>,
}

/// RAII guard that aborts an in-flight Copilot turn when the chat
/// stream is dropped without seeing `Done`/`Error`. Without this, a
/// caller that drops the stream early leaks a generating turn —
/// continued cost, possibly mis-attributed `assistant.usage` events,
/// and the next `send` racing the dying turn's residual events.
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
        // We're in a sync `Drop`; spawn the async abort. Failures here
        // are best-effort — the worst case is we waste tokens on a
        // turn the caller doesn't want.
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
    pub(crate) fn new(session: Arc<Session>) -> Arc<Self> {
        Arc::new(Self {
            session,
            submit_lock: Arc::new(Mutex::new(())),
        })
    }

    pub(crate) async fn shutdown(&self) -> Result<(), WorkerError> {
        // Disconnect (not destroy) — preserves on-disk state so the
        // session resumes on next daemon start (within the same date).
        self.session.disconnect().await?;
        Ok(())
    }
}

#[async_trait]
impl ChatWorker for CopilotChatWorker {
    async fn send(
        &self,
        prompt: ChatPrompt,
    ) -> Result<Pin<Box<dyn Stream<Item = ChatDelta> + Send>>, WorkerError> {
        let session = Arc::clone(&self.session);
        let lock = Arc::clone(&self.submit_lock);

        // Tracks whether the stream completed naturally (saw `Done` or
        // `Error`). Cleared in the success path; the AbortGuard's Drop
        // checks this and fires `session.abort()` only if the stream is
        // dropped mid-turn (caller cancelled). Without this, a dropped
        // stream leaks an in-flight Copilot turn — wasted quota + drift.
        let abort_guard = AbortGuard {
            session: Arc::clone(&session),
            completed: Arc::new(AtomicBool::new(false)),
        };
        let completed = Arc::clone(&abort_guard.completed);

        let s = stream! {
            // Move the AbortGuard into the stream. On stream drop, its
            // Drop impl fires abort if `completed` is still false.
            let _abort_on_drop = abort_guard;
            // Acquire lock inside the stream so a queued caller waits
            // cleanly, and releases on completion / drop.
            let _guard = lock.lock_owned().await;

            let mut events = session.subscribe();

            let opts = match build_message_options(prompt, WorkerClass::Chat) {
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

    async fn abort(&self) -> Result<(), WorkerError> {
        self.session.abort().await?;
        Ok(())
    }

    async fn compact(&self) -> Result<(), WorkerError> {
        // Wire method: `session.history.compact`. Marked experimental in
        // the SDK — pin both SDK + CLI versions if behavior changes.
        self.session.rpc().history().compact().await?;
        Ok(())
    }
}

/// Build SDK `MessageOptions` from a `ChatPrompt`. Maps each
/// `ChatAttachment` to the right `Attachment` variant — blob for
/// in-memory bytes (preferred for screenshots), file for paths.
fn build_message_options(
    prompt: ChatPrompt,
    class: WorkerClass,
) -> Result<MessageOptions, std::io::Error> {
    let mut attachments = Vec::with_capacity(prompt.attachments.len());
    for att in prompt.attachments {
        attachments.push(to_sdk_attachment(att)?);
    }

    // Session mode (interactive) set at session-create time on
    // `SessionConfig`. `MessageOptions::with_mode` is delivery-mode only.
    let _ = class; // accepted for signature symmetry; mode lives on the session
    let mut opts = MessageOptions::new(prompt.text);
    if !attachments.is_empty() {
        opts = opts.with_attachments(attachments);
    }
    Ok(opts)
}

fn to_sdk_attachment(att: ChatAttachment) -> Result<Attachment, std::io::Error> {
    match att {
        ChatAttachment::ImageBytes { bytes, mime_type } => Ok(Attachment::Blob {
            data: BASE64.encode(&bytes),
            mime_type: mime_type.to_string(),
            display_name: None,
        }),
        ChatAttachment::File { path } => {
            // SDK requires absolute paths; fail loudly if we got a relative one.
            if !path.is_absolute() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("file attachment must be absolute: {}", path.display()),
                ));
            }
            Ok(Attachment::File {
                path,
                display_name: None,
                line_range: None,
            })
        }
    }
}

/// Map an SDK `SessionEvent` to a BoBe `ChatDelta`. Returns `None` for
/// events we don't surface (turn_start/turn_end, intent, reasoning,
/// usage — usage is observed by `BobeHandler`, not the chat stream).
fn event_to_delta(event: &SessionEvent) -> Option<ChatDelta> {
    match event.event_type.as_str() {
        "assistant.message_delta" => event
            .data
            .get("deltaContent")
            .and_then(|v| v.as_str())
            .map(|s| ChatDelta::MessageDelta(s.to_string())),

        "assistant.message" => {
            let content = event
                .data
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let output_tokens = event
                .data
                .get("outputTokens")
                .and_then(serde_json::Value::as_u64);
            Some(ChatDelta::MessageComplete {
                content,
                output_tokens,
            })
        }

        "tool.execution_start" => {
            let name = event
                .data
                .get("toolName")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let args = event
                .data
                .get("arguments")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            Some(ChatDelta::ToolStart { name, args })
        }

        "tool.execution_complete" => {
            let name = event
                .data
                .get("toolName")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let success = event
                .data
                .get("success")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            Some(ChatDelta::ToolComplete { name, success })
        }

        "session.idle" => Some(ChatDelta::Done),

        "session.error" => {
            let msg = event
                .data
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("session error")
                .to_string();
            Some(ChatDelta::Error(msg))
        }

        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "tests panic on precondition failures")]
mod tests {
    use super::*;
    use serde_json::json;

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
            json!({ "toolName": "bash", "arguments": { "cmd": "ls" } }),
        );
        match event_to_delta(&start) {
            Some(ChatDelta::ToolStart { name, args }) => {
                assert_eq!(name, "bash");
                assert_eq!(args["cmd"], "ls");
            }
            other => panic!("expected ToolStart, got {other:?}"),
        }
        let complete = ev(
            "tool.execution_complete",
            json!({ "toolName": "bash", "success": true }),
        );
        match event_to_delta(&complete) {
            Some(ChatDelta::ToolComplete { name, success }) => {
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
        };
        let opts = build_message_options(prompt, WorkerClass::Vision).unwrap();
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
    fn build_options_rejects_relative_file_path() {
        let prompt = ChatPrompt {
            text: "hi".into(),
            attachments: vec![ChatAttachment::File {
                path: std::path::PathBuf::from("relative/path.png"),
            }],
        };
        let err = build_message_options(prompt, WorkerClass::Vision).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }
}
