use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::stream::Stream;
use tokio_stream::wrappers::ReceiverStream;

use crate::app_state::AppState;

/// GET /api/events
///
/// SSE endpoint for real-time event streaming. Uses ConnectionManager
/// to track client connections and handle reconnection with stale event
/// trimming. Single-consumer model — only one client at a time.
pub async fn stream_events(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let connection_manager = state.connection_manager.clone();
    let queue = state.event_queue.clone();
    let runtime_session = state.runtime_session.clone();

    // Establish connection — ConnectionManager sends initial indicator event
    // and fires the on_connect callback (which calls runtime_session.on_connection)
    let conn_id = connection_manager.connect().await;
    tracing::info!(connection_id = %conn_id, "sse.connected");

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(64);

    let conn_id_inner = conn_id.clone();
    let cm = connection_manager.clone();
    let rs = runtime_session.clone();
    tokio::spawn(async move {
        loop {
            // Check if this connection is still active
            if !cm.is_active_connection(&conn_id_inner).await {
                tracing::info!(
                    connection_id = %conn_id_inner,
                    "sse.connection_replaced"
                );
                rs.on_disconnection().await;
                break;
            }

            // Pop with timeout to periodically re-check connection liveness
            let event = tokio::time::timeout(Duration::from_secs(1), queue.pop()).await;

            match event {
                Ok(bundle) => {
                    // Track indicator state
                    cm.track_indicator(&bundle).await;

                    let sse_data = serde_json::to_string(&bundle).unwrap_or_default();
                    let sse_event = Event::default().event("message").data(sse_data);
                    if tx.send(Ok(sse_event)).await.is_err() {
                        tracing::info!(
                            connection_id = %conn_id_inner,
                            "sse.client_disconnected"
                        );
                        cm.disconnect(Some(&conn_id_inner)).await;
                        rs.on_disconnection().await;
                        break;
                    }
                }
                Err(_) => {
                    // Timeout — loop will re-check connection liveness
                    continue;
                }
            }
        }
    });

    Sse::new(ReceiverStream::new(rx))
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}
