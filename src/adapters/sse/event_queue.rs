use std::collections::VecDeque;
use std::sync::Mutex;
use tokio::sync::Notify;

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
        let mut queue = self.inner.lock().unwrap();
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
            {
                let mut queue = self.inner.lock().unwrap();
                if let Some(event) = queue.pop_front() {
                    return event;
                }
            }
            self.notify.notified().await;
        }
    }

    /// Get the current indicator state (for reconnection).
    pub fn current_indicator(&self) -> IndicatorType {
        *self.current_indicator.lock().unwrap()
    }

    /// Set the current indicator state.
    pub fn set_indicator(&self, indicator: IndicatorType) {
        *self.current_indicator.lock().unwrap() = indicator;
    }

    /// Number of events currently in the queue.
    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.lock().unwrap().is_empty()
    }

    /// Push a heartbeat event.
    pub fn push_heartbeat(&self) {
        use super::types::EventType;
        let event = StreamBundle {
            event_type: EventType::Heartbeat,
            message_id: String::new(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            description: "heartbeat".to_owned(),
            payload: serde_json::json!({}),
        };
        self.push(event);
    }
}
