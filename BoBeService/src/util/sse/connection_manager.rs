use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info};
use uuid::Uuid;

use super::event_queue::EventQueue;

const STALE_THRESHOLD_SECONDS: i64 = 60;

pub(crate) struct SseConnectionManager {
    queue: Arc<EventQueue>,
    on_connect: RwLock<Option<Box<dyn Fn() + Send + Sync>>>,
    on_disconnect: RwLock<Option<Box<dyn Fn() + Send + Sync>>>,
    state: Mutex<ConnectionState>,
}

struct ConnectionState {
    connected: bool,
    connection_id: Option<String>,
    generation: u64,
    disconnect_time: Option<DateTime<Utc>>,
}

impl SseConnectionManager {
    pub(crate) fn new(
        queue: Arc<EventQueue>,
        on_connect: Option<Box<dyn Fn() + Send + Sync>>,
        on_disconnect: Option<Box<dyn Fn() + Send + Sync>>,
    ) -> Self {
        Self {
            queue,
            on_connect: RwLock::new(on_connect),
            on_disconnect: RwLock::new(on_disconnect),
            state: Mutex::new(ConnectionState {
                connected: false,
                connection_id: None,
                generation: 0,
                disconnect_time: None,
            }),
        }
    }

    pub(crate) async fn set_callbacks(
        &self,
        on_connect: Box<dyn Fn() + Send + Sync>,
        on_disconnect: Box<dyn Fn() + Send + Sync>,
    ) {
        *self.on_connect.write().await = Some(on_connect);
        *self.on_disconnect.write().await = Some(on_disconnect);
    }

    pub(crate) async fn connect(&self) -> String {
        let mut st = self.state.lock().await;

        if st.connected {
            info!(old_id = ?st.connection_id, "connection_manager.replacing_connection");
        }

        let was_disconnected = !st.connected;
        st.connected = true;
        st.generation += 1;
        let conn_id = format!("conn_{}", &Uuid::new_v4().to_string()[..8]);
        st.connection_id = Some(conn_id.clone());

        if was_disconnected && let Some(disconnect_time) = st.disconnect_time {
            let disconnect_duration = (Utc::now() - disconnect_time).num_seconds();
            info!(
                disconnect_seconds = disconnect_duration,
                connection_id = %conn_id,
                "connection_manager.reconnected"
            );

            if disconnect_duration >= STALE_THRESHOLD_SECONDS {
                drop(st);
                self.trim_stale_events().await;
                let mut st = self.state.lock().await;
                st.disconnect_time = None;

                drop(st);
                self.queue.replay_current_indicator();

                info!(connection_id = %conn_id, "connection_manager.connected");
                if let Some(cb) = self.on_connect.read().await.as_ref() {
                    cb();
                }
                return conn_id;
            }
        }

        st.disconnect_time = None;
        drop(st);

        self.queue.replay_current_indicator();
        info!(connection_id = %conn_id, "connection_manager.connected");

        if let Some(cb) = self.on_connect.read().await.as_ref() {
            cb();
        }

        conn_id
    }

    pub(crate) async fn disconnect(&self, connection_id: Option<&str>) {
        let mut st = self.state.lock().await;

        if let Some(cid) = connection_id
            && st.connection_id.as_deref() != Some(cid)
        {
            debug!(
                stale_id = cid,
                current_id = ?st.connection_id,
                "connection_manager.ignored_stale_disconnect"
            );
            return;
        }

        st.connected = false;
        st.disconnect_time = Some(Utc::now());
        info!(connection_id = ?connection_id, "connection_manager.disconnected");

        drop(st);
        if let Some(cb) = self.on_disconnect.read().await.as_ref() {
            cb();
        }
    }

    pub(crate) async fn is_active_connection(&self, connection_id: &str) -> bool {
        let st = self.state.lock().await;
        st.connected && st.connection_id.as_deref() == Some(connection_id)
    }

    async fn trim_stale_events(&self) {
        let events = self.queue.clear();
        let cutoff = Utc::now() - Duration::seconds(STALE_THRESHOLD_SECONDS);
        let mut trimmed_count = 0;

        for event in events {
            if let Ok(ts) = DateTime::parse_from_rfc3339(&event.timestamp) {
                if ts >= cutoff {
                    self.queue.push(event);
                } else {
                    trimmed_count += 1;
                }
            } else {
                self.queue.push(event);
            }
        }

        if trimmed_count > 0 {
            info!(count = trimmed_count, "connection_manager.trimmed_stale");
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::util::sse::types::IndicatorType;

    #[tokio::test]
    async fn connect_replays_event_queue_indicator() {
        let queue = Arc::new(EventQueue::new(100));
        queue.set_indicator(IndicatorType::Thinking);
        queue.clear();
        let manager = SseConnectionManager::new(Arc::clone(&queue), None, None);

        manager.connect().await;
        let events = queue.clear();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].payload["indicator"], "THINKING");
    }
}
