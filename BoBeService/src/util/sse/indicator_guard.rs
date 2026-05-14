//! RAII guard that resets the SSE indicator to `Idle` on drop. Critical
//! for any code path that flips the indicator (e.g., to `Streaming`) and
//! could be aborted mid-flight via `JoinHandle::abort()` or panic-unwind.
//!
//! Without this guard, a `set_indicator(Idle)` at function end is skipped
//! on abort, leaving the indicator stuck non-`Idle`. The next
//! `try_begin_user_message` then rejects every turn with
//! "BoBe is still responding".

use std::sync::Arc;

use super::event_queue::EventQueue;
use super::types::IndicatorType;

/// Holds an `Arc<EventQueue>` so its `Drop` impl can fire from any
/// scope, including detached tokio tasks that are aborted mid-flight.
pub(crate) struct IndicatorGuard {
    queue: Arc<EventQueue>,
}

impl IndicatorGuard {
    pub(crate) fn new(queue: Arc<EventQueue>) -> Self {
        Self { queue }
    }
}

impl Drop for IndicatorGuard {
    fn drop(&mut self) {
        self.queue.set_indicator(IndicatorType::Idle);
    }
}
