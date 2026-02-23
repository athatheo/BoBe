use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::error::AppError;

// ── Request / Response ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ConversationMessageRequest {
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct ConversationMessageResponse {
    pub message_id: String,
}

// ── Handler ─────────────────────────────────────────────────────────────────

/// POST /api/conversation/message
///
/// Receives a user message and delegates to the runtime session's message
/// handler. The LLM response streams back via SSE events. Returns the
/// message ID immediately so the client can correlate SSE events.
pub async fn send_message(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ConversationMessageRequest>,
) -> Result<Json<ConversationMessageResponse>, AppError> {
    if body.content.is_empty() {
        return Err(AppError::Validation("content must not be empty".into()));
    }

    // Delegate to the runtime session which handles:
    // 1. Conversation lifecycle (create/activate)
    // 2. Learning (embed user message)
    // 3. LLM response generation (streamed via SSE)
    let session = state.runtime_session.clone();
    let content = body.content.clone();

    // Generate message ID upfront so it can be returned immediately
    let message_id = format!("msg_{}", uuid::Uuid::new_v4().simple());
    let msg_id = message_id.clone();

    // Fire-and-forget: spawn the message handling so HTTP returns immediately.
    // The response streams via SSE events to the client.
    tokio::spawn(async move {
        session.handle_user_message(&content, &msg_id).await;
    });

    tracing::info!(message_id = %message_id, "api.message_accepted");

    Ok(Json(ConversationMessageResponse { message_id }))
}
