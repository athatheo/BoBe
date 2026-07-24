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

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::sync::Arc;

    use super::*;

    #[tokio::test]
    async fn task_abort_resets_indicator() {
        let queue = Arc::new(EventQueue::new(4));
        let task_queue = Arc::clone(&queue);
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            task_queue.set_indicator(IndicatorType::ScreenCapture);
            let _guard = IndicatorGuard::new(Arc::clone(&task_queue));
            assert!(ready_tx.send(()).is_ok());
            std::future::pending::<()>().await;
        });

        ready_rx.await.expect("task should install indicator guard");
        assert_eq!(queue.current_indicator(), IndicatorType::ScreenCapture);

        task.abort();
        let join_error = task.await.expect_err("task should be cancelled");
        assert!(join_error.is_cancelled());
        assert_eq!(queue.current_indicator(), IndicatorType::Idle);
    }
}
