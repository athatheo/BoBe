use std::collections::VecDeque;
use std::sync::Mutex;
use tokio::sync::Notify;

use super::factories::{heartbeat_event, indicator_event};
use super::types::{IndicatorType, StreamBundle};

/// Bounded async queue for SSE events.
///
/// Properties:
/// - Max size: configurable (default 100)
/// - Overflow: drops oldest events
/// - No persistence (ephemeral)
pub struct EventQueue {
    inner: Mutex<VecDeque<StreamBundle>>,
    max_size: usize,
    notify: Notify,
    current_indicator: Mutex<IndicatorType>,
}

impl EventQueue {
    pub fn new(max_size: usize) -> Self {
        Self {
            inner: Mutex::new(VecDeque::with_capacity(max_size)),
            max_size,
            notify: Notify::new(),
            current_indicator: Mutex::new(IndicatorType::default()),
        }
    }

    /// Push an event into the queue. Drops oldest if full.
    pub fn push(&self, event: StreamBundle) {
        let mut queue = lock_or_recover(&self.inner, "event_queue.inner");
        if queue.len() >= self.max_size {
            queue.pop_front();
            tracing::warn!("SSE event queue overflow, dropping oldest event");
        }
        queue.push_back(event);
        drop(queue);
        self.notify.notify_waiters();
    }

    /// Pop the next event, or wait until one is available.
    pub async fn pop(&self) -> StreamBundle {
        loop {
            // Register for notification BEFORE checking queue to avoid race
            let notified = self.notify.notified();
            {
                let mut queue = lock_or_recover(&self.inner, "event_queue.inner");
                if let Some(event) = queue.pop_front() {
                    return event;
                }
            }
            notified.await;
        }
    }

    /// Get the current indicator state (for reconnection).
    pub fn current_indicator(&self) -> IndicatorType {
        *lock_or_recover(&self.current_indicator, "event_queue.current_indicator")
    }

    /// Set the current indicator state and push an indicator event.
    pub fn set_indicator(&self, indicator: IndicatorType) {
        *lock_or_recover(&self.current_indicator, "event_queue.current_indicator") = indicator;
        self.push(indicator_event(indicator, None));
    }

    /// Number of events currently in the queue.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        lock_or_recover(&self.inner, "event_queue.inner").len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        lock_or_recover(&self.inner, "event_queue.inner").is_empty()
    }

    /// Drain all events from the queue.
    pub fn clear(&self) -> Vec<StreamBundle> {
        let mut queue = lock_or_recover(&self.inner, "event_queue.inner");
        queue.drain(..).collect()
    }

    /// Push a heartbeat event.
    pub fn push_heartbeat(&self) {
        self.push(heartbeat_event());
    }
}

fn lock_or_recover<'a, T>(
    mutex: &'a Mutex<T>,
    lock_name: &'static str,
) -> std::sync::MutexGuard<'a, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            tracing::error!(lock = lock_name, "mutex poisoned, recovering");
            poisoned.into_inner()
        }
    }
}
