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
/// SSE endpoint for real-time event streaming. Streams events from the
/// EventQueue to the connected client. Uses `event: message` with JSON
/// payload discriminated by `type` field.
pub async fn stream_events(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let queue = state.event_queue.clone();

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(64);

    // Send initial indicator event
    let indicator = queue.current_indicator();
    let init_event = Event::default()
        .event("message")
        .json_data(serde_json::json!({
            "type": "indicator",
            "message_id": "",
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "description": "Current indicator",
            "payload": { "indicator": indicator },
        }))
        .unwrap_or_else(|_| Event::default().data("{}"));

    let _ = tx.send(Ok(init_event)).await;

    tokio::spawn(async move {
        loop {
            let event = queue.pop().await;
            let sse_data = serde_json::to_string(&event).unwrap_or_default();
            let sse_event = Event::default().event("message").data(sse_data);
            if tx.send(Ok(sse_event)).await.is_err() {
                // Client disconnected
                tracing::info!("sse.client_disconnected");
                break;
            }
        }
    });

    Sse::new(ReceiverStream::new(rx))
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}
