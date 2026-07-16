use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, Instant};

use futures::{Stream, StreamExt};
use tracing::{debug, error, info, warn};

use crate::constants::MILLIS_PER_SECOND;
use crate::copilot::types::ChatDelta;
use crate::models::ids::new_message_id;
use crate::util::sse::event_queue::EventQueue;
use crate::util::sse::factories::{
    end_of_turn_event, error_event, text_delta_event, tool_call_complete_event,
    tool_call_start_event,
};
use crate::voice::telemetry::HIST_LLM_TTFT_MS;

/// Inter-token gap above which we consider the LLM stream stalled. Production
/// pattern (LiveKit, Pipecat) treats >500-800ms gaps as a stall worth audible
/// feedback. Today logs only — filler-on-stall lives in the voice path via
/// `filler_watchdog`, this is purely an observability hook.
const STALL_THRESHOLD: Duration = Duration::from_millis(800);
const STREAM_IDLE_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug)]
pub(crate) struct StreamResult {
    pub(crate) full_response: String,
    pub(crate) chunk_count: usize,
    pub(crate) duration_ms: f64,
    pub(crate) success: bool,
    pub(crate) first_token_ms: Option<f64>,
}

impl StreamResult {
    /// Chunks-per-second for completion logs; `0.0` when duration is zero
    /// (an empty stream — the divide-by-zero guard).
    pub(crate) fn chunks_per_sec(&self) -> f64 {
        if self.duration_ms > 0.0 {
            self.chunk_count as f64 / (self.duration_ms / MILLIS_PER_SECOND)
        } else {
            0.0
        }
    }
}

struct StreamAccumulator {
    msg_id: String,
    start_time: Instant,
    sequence: usize,
    full_response: String,
    success: bool,
    first_token_time: Option<Instant>,
}

impl StreamAccumulator {
    fn new(msg_id: Option<&str>) -> Self {
        Self {
            msg_id: msg_id.map_or_else(new_message_id, str::to_owned),
            start_time: Instant::now(),
            sequence: 0,
            full_response: String::new(),
            success: true,
            first_token_time: None,
        }
    }

    fn msg_id(&self) -> &str {
        &self.msg_id
    }

    fn mark_failed(&mut self) {
        self.success = false;
    }

    fn finish(self, event_queue: &EventQueue) -> StreamResult {
        event_queue.push(end_of_turn_event(
            &self.msg_id,
            self.sequence,
            &self.full_response,
        ));

        let duration_ms = self.start_time.elapsed().as_secs_f64() * MILLIS_PER_SECOND;
        let first_token_ms = self
            .first_token_time
            .map(|t| (t - self.start_time).as_secs_f64() * MILLIS_PER_SECOND);
        if let Some(ttft) = first_token_ms {
            metrics::histogram!(HIST_LLM_TTFT_MS).record(ttft);
        }

        StreamResult {
            full_response: self.full_response,
            chunk_count: self.sequence,
            duration_ms,
            success: self.success,
            first_token_ms,
        }
    }
}

pub(crate) async fn stream_chat_delta_response<F, Fut>(
    mut stream: Pin<Box<dyn Stream<Item = ChatDelta> + Send>>,
    event_queue: &EventQueue,
    msg_id: Option<&str>,
    mut on_text_delta: F,
) -> StreamResult
where
    F: FnMut(String) -> Fut + Send,
    Fut: Future<Output = ()> + Send,
{
    let mut state = StreamAccumulator::new(msg_id);
    let mut last_token_at: Option<Instant> = None;
    let mut stall_count: u32 = 0;

    loop {
        let delta = match tokio::time::timeout(STREAM_IDLE_TIMEOUT, stream.next()).await {
            Ok(Some(delta)) => delta,
            Ok(None) => break,
            Err(_) => {
                state.mark_failed();
                error!(
                    msg_id = state.msg_id(),
                    timeout_ms = STREAM_IDLE_TIMEOUT.as_millis() as u64,
                    "stream_chat_delta.idle_timeout"
                );
                event_queue.push(error_event(
                    state.msg_id(),
                    "CHAT_STREAM_TIMEOUT",
                    "Assistant stream stopped responding",
                    true,
                ));
                break;
            }
        };
        match delta {
            ChatDelta::MessageDelta(text) => {
                if !text.is_empty() {
                    let now = Instant::now();
                    if state.first_token_time.is_none() {
                        state.first_token_time = Some(now);
                    }
                    // Stall watchdog: emit a structured log every time the
                    // inter-token gap exceeds the threshold. The voice path
                    // fires its own filler-watchdog in parallel.
                    if let Some(prev) = last_token_at {
                        let gap = now.duration_since(prev);
                        if gap > STALL_THRESHOLD {
                            stall_count = stall_count.saturating_add(1);
                            warn!(
                                msg_id = state.msg_id(),
                                gap_ms = gap.as_millis() as u64,
                                stall_count,
                                "voice.stream.inter_token_stall"
                            );
                        }
                    }
                    last_token_at = Some(now);

                    state.full_response.push_str(&text);
                    on_text_delta(text.clone()).await;
                    event_queue.push(text_delta_event(
                        state.msg_id(),
                        &text,
                        state.sequence,
                        false,
                    ));
                    state.sequence += 1;
                }
            }
            ChatDelta::MessageComplete {
                content,
                output_tokens,
            } => {
                debug!(
                    bytes = content.len(),
                    output_tokens = ?output_tokens,
                    "stream_chat_delta.message_complete"
                );
                // Non-streaming SDK paths emit no deltas; surface the full content as one.
                if state.full_response.is_empty() && !content.is_empty() {
                    if state.first_token_time.is_none() {
                        state.first_token_time = Some(Instant::now());
                    }
                    on_text_delta(content.clone()).await;
                    event_queue.push(text_delta_event(
                        state.msg_id(),
                        &content,
                        state.sequence,
                        false,
                    ));
                    state.full_response = content;
                    state.sequence += 1;
                } else if content != state.full_response {
                    if let Some(missing) = content.strip_prefix(&state.full_response) {
                        if !missing.is_empty() {
                            on_text_delta(missing.to_owned()).await;
                            event_queue.push(text_delta_event(
                                state.msg_id(),
                                missing,
                                state.sequence,
                                false,
                            ));
                            state.sequence += 1;
                        }
                    } else {
                        warn!(
                            accumulated_bytes = state.full_response.len(),
                            final_bytes = content.len(),
                            "stream_chat_delta.final_content_diverged"
                        );
                    }
                    // The terminal event is authoritative for persistence even
                    // when a lagged subscription dropped or rewrote deltas.
                    state.full_response = content;
                }
            }
            ChatDelta::ToolStart { id, name } => {
                info!(tool = %name, "tool_call.start");
                event_queue.push(tool_call_start_event(state.msg_id(), &name, &id));
            }
            ChatDelta::ToolComplete { id, name, success } => {
                info!(tool = %name, success, "tool_call.complete");
                event_queue.push(tool_call_complete_event(
                    state.msg_id(),
                    &name,
                    &id,
                    Some(success),
                    None,
                    None,
                ));
            }
            ChatDelta::Error(msg) => {
                state.mark_failed();
                error!(error = %msg, chunks = state.sequence, "stream_chat_delta.error");
                event_queue.push(error_event(state.msg_id(), "CHAT_ERROR", &msg, true));
            }
            ChatDelta::Done => {
                debug!(chunks = state.sequence, "stream_chat_delta.done");
                break;
            }
        }
    }

    state.finish(event_queue)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::sync::{Arc, Mutex};

    use futures::stream;

    use super::*;

    #[tokio::test]
    async fn terminal_content_repairs_lagged_suffix() {
        let events = vec![
            ChatDelta::MessageDelta("Hel".into()),
            ChatDelta::MessageComplete {
                content: "Hello".into(),
                output_tokens: Some(1),
            },
            ChatDelta::Done,
        ];
        let observed = Arc::new(Mutex::new(String::new()));
        let observed_for_callback = Arc::clone(&observed);
        let queue = EventQueue::new(16);

        let result = stream_chat_delta_response(
            Box::pin(stream::iter(events)),
            &queue,
            Some("msg"),
            move |delta| {
                let observed = Arc::clone(&observed_for_callback);
                async move {
                    observed
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .push_str(&delta);
                }
            },
        )
        .await;

        assert_eq!(result.full_response, "Hello");
        let terminal = queue.clear().pop().expect("terminal event should exist");
        assert_eq!(terminal.payload["done"], true);
        assert_eq!(terminal.payload["delta"], "Hello");
        assert_eq!(
            *observed
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            "Hello"
        );
    }

    #[tokio::test]
    async fn terminal_event_survives_queue_overflow_with_full_response() {
        let events = vec![
            ChatDelta::MessageDelta("first".into()),
            ChatDelta::MessageDelta(" second".into()),
            ChatDelta::Done,
        ];
        let queue = EventQueue::new(1);

        let result = stream_chat_delta_response(
            Box::pin(stream::iter(events)),
            &queue,
            Some("msg"),
            |_| async {},
        )
        .await;

        let queued = queue.clear();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].payload["done"], true);
        assert_eq!(queued[0].payload["delta"], "first second");
        assert_eq!(result.full_response, "first second");
    }

    #[tokio::test]
    async fn terminal_content_is_authoritative_when_deltas_diverge() {
        let events = vec![
            ChatDelta::MessageDelta("draft".into()),
            ChatDelta::MessageComplete {
                content: "final".into(),
                output_tokens: Some(1),
            },
            ChatDelta::Done,
        ];
        let queue = EventQueue::new(16);

        let result = stream_chat_delta_response(
            Box::pin(stream::iter(events)),
            &queue,
            Some("msg"),
            |_| async {},
        )
        .await;

        assert_eq!(result.full_response, "final");
    }
}
