use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::error::AppError;

/// RAII counter guard. Increments on construction; decrements on drop
/// (including panic-unwind). Used to track in-flight text-turn tasks so
/// the graceful-shutdown path can wait for them before closing the DB
/// pool — otherwise the spawned task panics mid-sqlx call.
struct InFlightCounter(Arc<AtomicUsize>);

impl InFlightCounter {
    fn new(counter: Arc<AtomicUsize>) -> Self {
        counter.fetch_add(1, Ordering::AcqRel);
        Self(counter)
    }
}

impl Drop for InFlightCounter {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct ConversationMessageRequest {
    pub(crate) content: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct ConversationMessageResponse {
    pub(crate) message_id: String,
}

pub(crate) async fn send_message(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ConversationMessageRequest>,
) -> Result<Json<ConversationMessageResponse>, AppError> {
    if body.content.trim().is_empty() {
        return Err(AppError::Validation("content must not be empty".into()));
    }

    let session = Arc::clone(&state.runtime_session);
    let content = body.content.clone();
    let user_message_guard = session
        .try_begin_user_message()
        .map_err(|message| AppError::Conflict(message.into()))?;

    let message_id = format!("msg_{}", uuid::Uuid::new_v4().simple());
    let msg_id = message_id.clone();
    let in_flight = InFlightCounter::new(Arc::clone(&state.in_flight_text_turns));

    tokio::spawn(async move {
        let _user_message_guard = user_message_guard;
        let _in_flight = in_flight;
        session.handle_user_message(&content, &msg_id).await;
    });

    tracing::info!(message_id = %message_id, "api.message_accepted");

    Ok(Json(ConversationMessageResponse { message_id }))
}
