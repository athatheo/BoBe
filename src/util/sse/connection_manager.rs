use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info};
use uuid::Uuid;

use super::event_queue::EventQueue;
use super::factories::indicator_event;
use super::types::{IndicatorType, StreamBundle};

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
    current_indicator: IndicatorType,
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
                current_indicator: IndicatorType::Idle,
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

    pub(crate) async fn track_indicator(&self, event: &StreamBundle) {
        if event.event_type == super::types::EventType::Indicator
            && let Some(ind) = event.payload.get("indicator").and_then(|v| v.as_str())
        {
            let mut st = self.state.lock().await;
            st.current_indicator = match ind {
                "SCREEN_CAPTURE" | "screen_capture" | "ScreenCapture" => {
                    IndicatorType::ScreenCapture
                }
                "THINKING" | "thinking" | "Thinking" => IndicatorType::Thinking,
                "TOOL_CALLING" | "tool_calling" | "ToolCalling" => IndicatorType::ToolCalling,
                "STREAMING" | "streaming" | "Streaming" => IndicatorType::Streaming,
                _ => IndicatorType::Idle,
            };
        }
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

                let indicator = st.current_indicator;
                drop(st);
                self.queue.push(indicator_event(indicator, None));

                info!(connection_id = %conn_id, "connection_manager.connected");
                if let Some(cb) = self.on_connect.read().await.as_ref() {
                    cb();
                }
                return conn_id;
            }
        }

        st.disconnect_time = None;
        let indicator = st.current_indicator;
        drop(st);

        self.queue.push(indicator_event(indicator, None));
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

    #[cfg(test)]
    pub(crate) async fn current_indicator(&self) -> IndicatorType {
        self.state.lock().await.current_indicator
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
mod tests {
    use super::*;
    use crate::util::sse::types::{EventType, StreamBundle};
    use serde_json::json;

    #[tokio::test]
    async fn track_indicator_accepts_screaming_snake_case() {
        let queue = Arc::new(EventQueue::new(100));
        let manager = SseConnectionManager::new(queue, None, None);
        let bundle = StreamBundle {
            event_type: EventType::Indicator,
            message_id: String::new(),
            timestamp: Utc::now().to_rfc3339(),
            description: String::new(),
            payload: json!({ "indicator": "TOOL_CALLING" }),
        };

        manager.track_indicator(&bundle).await;

        assert_eq!(
            manager.current_indicator().await,
            IndicatorType::ToolCalling
        );
    }
}
