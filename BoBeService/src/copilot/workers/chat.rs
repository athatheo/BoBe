use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use async_stream::stream;
use futures::Stream;
use github_copilot_sdk::rpc::MetadataContextHeaviestMessagesRequest;
use github_copilot_sdk::session::Session;
use github_copilot_sdk::session_events::{
    AssistantMessageData, AssistantMessageDeltaData, SessionErrorData, SessionEventType,
    ToolExecutionCompleteData, ToolExecutionStartData,
};
use github_copilot_sdk::subscription::RecvErrorKind;
use github_copilot_sdk::types::{DeliveryMode, MessageOptions, SessionEvent, SetModelOptions};
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;

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
    text_reasoning_effort: Option<String>,
    /// Last effort we sent via set_model; skip the redundant RPC if the
    /// new send wants the same effort. None = never called set_model.
    last_effort: Arc<Mutex<Option<String>>>,
    completed_turns: Arc<AtomicU64>,
    context_diagnostics_task: Arc<Mutex<Option<JoinHandle<()>>>>,
    lifecycle: Arc<RwLock<()>>,
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
    pub(crate) fn new(
        session: Arc<Session>,
        model: Option<String>,
        text_reasoning_effort: Option<String>,
        lifecycle: Arc<RwLock<()>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            session,
            submit_lock: Arc::new(Mutex::new(())),
            model,
            text_reasoning_effort,
            last_effort: Arc::new(Mutex::new(None)),
            completed_turns: Arc::new(AtomicU64::new(0)),
            context_diagnostics_task: Arc::new(Mutex::new(None)),
            lifecycle,
        })
    }

    pub(crate) async fn shutdown(&self) -> Result<(), WorkerError> {
        if let Some(task) = self.context_diagnostics_task.lock().await.take() {
            task.abort();
            drop(task.await);
        }
        // disconnect (not destroy) preserves on-disk state for resume within the same date.
        self.session.disconnect().await?;
        Ok(())
    }

    pub(crate) fn session(&self) -> Arc<Session> {
        Arc::clone(&self.session)
    }

    pub(crate) async fn abort(&self) -> Result<(), WorkerError> {
        self.session.abort().await?;
        Ok(())
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
        let text_reasoning_effort = self.text_reasoning_effort.clone();
        let last_effort = Arc::clone(&self.last_effort);
        let voice_mode = prompt.voice_mode;
        let completed_turns = Arc::clone(&self.completed_turns);
        let context_diagnostics_task = Arc::clone(&self.context_diagnostics_task);
        let lifecycle = Arc::clone(&self.lifecycle);

        let abort_guard = AbortGuard {
            session: Arc::clone(&session),
            completed: Arc::new(AtomicBool::new(false)),
        };
        let completed = Arc::clone(&abort_guard.completed);

        let s = stream! {
            let _lifecycle = lifecycle.read_owned().await;
            let _abort_on_drop = abort_guard;
            let _guard = lock.lock_owned().await;

            let mut events = session.subscribe();

            // Tune reasoning_effort per turn — low for voice (TTFT-critical),
            // configured value or medium for text. Skipped when model is None
            // or when the effort hasn't changed since the last send.
            if let Some(model_name) = model.as_deref() {
                let want_effort =
                    desired_reasoning_effort(voice_mode, text_reasoning_effort.as_deref());
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
                            *last_effort.lock().await = Some(want_effort.to_owned());
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
                    Ok(event) if let Some(delta) = event_to_delta(&event) => {
                        let stop = matches!(delta, ChatDelta::Done | ChatDelta::Error(_));
                        if matches!(delta, ChatDelta::Done) {
                            let completed = completed_turns.fetch_add(1, Ordering::AcqRel) + 1;
                            if completed.is_multiple_of(10) {
                                schedule_context_diagnostics(
                                    Arc::clone(&session),
                                    Arc::clone(&context_diagnostics_task),
                                )
                                .await;
                            }
                        }
                        yield delta;
                        if stop {
                            completed.store(true, Ordering::Release);
                            return;
                        }
                    }
                    Ok(_) => {}

                    Err(e) => {
                        if let RecvErrorKind::Lagged(skipped) = e.kind() {
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

async fn schedule_context_diagnostics(
    session: Arc<Session>,
    slot: Arc<Mutex<Option<JoinHandle<()>>>>,
) {
    let previous = {
        let mut guard = slot.lock().await;
        if guard.as_ref().is_some_and(|task| !task.is_finished()) {
            return;
        }
        guard.take()
    };
    if let Some(previous) = previous {
        drop(previous.await);
    }

    let task = tokio::spawn(async move {
        let query = async {
            let attribution = session.rpc().metadata().get_context_attribution().await?;
            let heaviest = session
                .rpc()
                .metadata()
                .get_context_heaviest_messages(MetadataContextHeaviestMessagesRequest {
                    limit: Some(3),
                })
                .await?;
            Ok::<_, github_copilot_sdk::Error>((attribution, heaviest))
        };
        match tokio::time::timeout(std::time::Duration::from_secs(5), query).await {
            Ok(Ok((attribution, heaviest))) => {
                let compactions = attribution
                    .context_attribution
                    .as_ref()
                    .map_or(0, |context| context.compactions.count);
                let largest_message_tokens = heaviest
                    .messages
                    .first()
                    .map_or(0, |message| message.tokens);
                tracing::info!(
                    total_tokens = heaviest.total_tokens,
                    compactions,
                    largest_message_tokens,
                    "chat.context_diagnostics"
                );
            }
            Ok(Err(error)) => {
                tracing::debug!(err = %error, "chat.context_diagnostics_unavailable");
            }
            Err(_) => {
                tracing::debug!("chat.context_diagnostics_timeout");
            }
        }
    });
    *slot.lock().await = Some(task);
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

fn desired_reasoning_effort(voice_mode: bool, configured_text: Option<&str>) -> &str {
    if voice_mode {
        "low"
    } else {
        configured_text.unwrap_or("medium")
    }
}

fn event_to_delta(event: &SessionEvent) -> Option<ChatDelta> {
    match event.parsed_type() {
        SessionEventType::AssistantMessageDelta => {
            let p = event.typed_data::<AssistantMessageDeltaData>()?;
            if p.delta_content.is_empty() {
                None
            } else {
                Some(ChatDelta::MessageDelta(p.delta_content))
            }
        }
        SessionEventType::AssistantMessage => {
            let p = event.typed_data::<AssistantMessageData>()?;
            Some(ChatDelta::MessageComplete {
                content: p.content,
                output_tokens: p
                    .output_tokens
                    .and_then(|tokens| u64::try_from(tokens).ok()),
            })
        }
        SessionEventType::ToolExecutionStart => {
            let p = event.typed_data::<ToolExecutionStartData>()?;
            Some(ChatDelta::ToolStart {
                id: p.tool_call_id,
                name: p.tool_name,
            })
        }
        SessionEventType::ToolExecutionComplete => {
            let p = event.typed_data::<ToolExecutionCompleteData>()?;
            let name = event
                .data
                .get("toolName")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .or_else(|| p.tool_description.map(|tool| tool.name))
                .unwrap_or_default();
            Some(ChatDelta::ToolComplete {
                id: p.tool_call_id,
                name,
                success: p.success,
            })
        }
        SessionEventType::SessionIdle => Some(ChatDelta::Done),
        SessionEventType::SessionError => {
            let p = event.typed_data::<SessionErrorData>()?;
            Some(ChatDelta::Error(p.message))
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
        std::assert_matches!(event_to_delta(&idle), Some(ChatDelta::Done));

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

    #[test]
    fn configured_text_reasoning_preserves_low_latency_voice() {
        assert_eq!(desired_reasoning_effort(false, Some("high")), "high");
        assert_eq!(desired_reasoning_effort(true, Some("high")), "low");
        assert_eq!(desired_reasoning_effort(false, None), "medium");
    }
}
