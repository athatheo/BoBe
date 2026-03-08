use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::stream::Stream;
use tokio_stream::wrappers::ReceiverStream;

use crate::app_state::AppState;

/// SSE endpoint. Single-consumer; `ConnectionManager` handles reconnection.
pub(crate) async fn stream_events(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let connection_manager = Arc::clone(&state.connection_manager);
    let queue = Arc::clone(&state.event_queue);

    let conn_id = connection_manager.connect().await;
    tracing::info!(connection_id = %conn_id, "sse.connected");

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(64);

    let conn_id_inner = conn_id.clone();
    let cm = Arc::clone(&connection_manager);
    tokio::spawn(async move {
        loop {
            if !cm.is_active_connection(&conn_id_inner).await {
                tracing::info!(
                    connection_id = %conn_id_inner,
                    "sse.connection_replaced"
                );
                break;
            }

            let event = tokio::time::timeout(Duration::from_secs(1), queue.pop()).await;

            if let Ok(bundle) = event {
                cm.track_indicator(&bundle).await;

                let sse_data = match serde_json::to_string(&bundle) {
                    Ok(json) => json,
                    Err(e) => {
                        tracing::warn!(error = %e, "sse.serialize_event_failed");
                        String::new()
                    }
                };
                let sse_event = Event::default().event("message").data(sse_data);
                if tx.send(Ok(sse_event)).await.is_err() {
                    tracing::info!(
                        connection_id = %conn_id_inner,
                        "sse.client_disconnected"
                    );
                    cm.disconnect(Some(&conn_id_inner)).await;
                    break;
                }
            }
        }
    });

    Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}
