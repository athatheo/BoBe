use std::sync::Arc;

use super::connection_manager::SseConnectionManager;
use super::event_queue::EventQueue;
use super::factories;
use super::types::StreamBundle;

/// Single-consumer event stream for local desktop app.
///
/// Composes EventQueue + SseConnectionManager. Designed for ONE SSE
/// client (the shell). If a second client connects, the first is
/// considered disconnected.
pub struct EventStream {
    queue: Arc<EventQueue>,
    connection_manager: Arc<SseConnectionManager>,
}

impl EventStream {
    pub fn new(queue: Arc<EventQueue>, connection_manager: Arc<SseConnectionManager>) -> Self {
        Self {
            queue,
            connection_manager,
        }
    }

    /// Push an event into the queue.
    pub fn push(&self, event: StreamBundle) {
        self.queue.push(event);
    }

    /// Whether a client is connected.
    pub async fn is_connected(&self) -> bool {
        self.connection_manager.is_connected().await
    }

    /// Get a reference to the underlying event queue.
    pub fn queue(&self) -> &Arc<EventQueue> {
        &self.queue
    }

    /// Run heartbeat loop as a background task.
    pub async fn run_heartbeat(&self, interval_seconds: u64) {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_seconds));
        loop {
            interval.tick().await;
            if self.is_connected().await {
                self.push(factories::heartbeat_event());
            }
        }
    }
}

/// Format a StreamBundle as an SSE text frame.
///
/// Returns a string like:
/// ```text
/// event: <event_type>
/// data: <json>
///
/// ```
pub fn format_sse_event(bundle: &StreamBundle) -> String {
    let data = serde_json::to_string(bundle).unwrap_or_default();
    format!("data: {data}\n\n")
}
