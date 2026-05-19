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

/// Inter-token gap above which we consider the LLM stream stalled. Production
/// pattern (LiveKit, Pipecat) treats >500-800ms gaps as a stall worth audible
/// feedback. Today logs only — filler-on-stall lives in the voice path via
/// `filler_watchdog`, this is purely an observability hook.
const STALL_THRESHOLD: Duration = Duration::from_millis(800);

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
        event_queue.push(end_of_turn_event(&self.msg_id, self.sequence));

        let duration_ms = self.start_time.elapsed().as_secs_f64() * MILLIS_PER_SECOND;
        let first_token_ms = self
            .first_token_time
            .map(|t| (t - self.start_time).as_secs_f64() * MILLIS_PER_SECOND);

        StreamResult {
            full_response: self.full_response,
            chunk_count: self.sequence,
            duration_ms,
            success: self.success,
            first_token_ms,
        }
    }
}

pub(crate) async fn stream_chat_delta_response<F>(
    mut stream: Pin<Box<dyn Stream<Item = ChatDelta> + Send>>,
    event_queue: &EventQueue,
    msg_id: Option<&str>,
    mut on_text_delta: F,
) -> StreamResult
where
    F: FnMut(&str) + Send,
{
    let mut state = StreamAccumulator::new(msg_id);
    let mut last_token_at: Option<Instant> = None;
    let mut stall_count: u32 = 0;

    while let Some(delta) = stream.next().await {
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
                    on_text_delta(&text);
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
                    on_text_delta(&content);
                    event_queue.push(text_delta_event(
                        state.msg_id(),
                        &content,
                        state.sequence,
                        false,
                    ));
                    state.full_response = content;
                    state.sequence += 1;
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
