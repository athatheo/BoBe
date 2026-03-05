use std::collections::VecDeque;
use std::sync::Mutex;
use tokio::sync::Notify;

use super::factories::{heartbeat_event, indicator_event};
use super::types::{IndicatorType, StreamBundle};

pub(crate) struct EventQueue {
    inner: Mutex<VecDeque<StreamBundle>>,
    max_size: usize,
    notify: Notify,
    current_indicator: Mutex<IndicatorType>,
}

impl EventQueue {
    pub(crate) fn new(max_size: usize) -> Self {
        Self {
            inner: Mutex::new(VecDeque::with_capacity(max_size)),
            max_size,
            notify: Notify::new(),
            current_indicator: Mutex::new(IndicatorType::default()),
        }
    }

    pub(crate) fn push(&self, event: StreamBundle) {
        let mut queue = lock_or_recover(&self.inner, "event_queue.inner");
        if queue.len() >= self.max_size && queue.pop_front_if(|_| true).is_some() {
            tracing::warn!("SSE event queue overflow, dropping oldest event");
        }
        queue.push_back(event);
        drop(queue);
        self.notify.notify_waiters();
    }

    pub(crate) async fn pop(&self) -> StreamBundle {
        loop {
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

    pub(crate) fn current_indicator(&self) -> IndicatorType {
        *lock_or_recover(&self.current_indicator, "event_queue.current_indicator")
    }

    pub(crate) fn set_indicator(&self, indicator: IndicatorType) {
        *lock_or_recover(&self.current_indicator, "event_queue.current_indicator") = indicator;
        self.push(indicator_event(indicator, None));
    }

    pub(crate) fn clear(&self) -> Vec<StreamBundle> {
        let mut queue = lock_or_recover(&self.inner, "event_queue.inner");
        queue.drain(..).collect()
    }

    pub(crate) fn push_heartbeat(&self) {
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
