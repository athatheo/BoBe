use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::error::AppError;

#[derive(Debug, Deserialize)]
pub(crate) struct ConversationMessageRequest {
    pub(crate) content: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct ConversationMessageResponse {
    pub(crate) message_id: String,
}

/// Returns message ID immediately; LLM response streams via SSE.
pub(crate) async fn send_message(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ConversationMessageRequest>,
) -> Result<Json<ConversationMessageResponse>, AppError> {
    if body.content.is_empty() {
        return Err(AppError::Validation("content must not be empty".into()));
    }

    let session = state.runtime_session.clone();
    let content = body.content.clone();

    let message_id = format!("msg_{}", uuid::Uuid::new_v4().simple());
    let msg_id = message_id.clone();

    tokio::spawn(async move {
        session.handle_user_message(&content, &msg_id).await;
    });

    tracing::info!(message_id = %message_id, "api.message_accepted");

    Ok(Json(ConversationMessageResponse { message_id }))
}

/// No-op; frontend handles dismiss locally.
pub(crate) async fn dismiss_message() -> axum::http::StatusCode {
    axum::http::StatusCode::OK
}
