use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::adapters::persistence::repos::conversation_repo::SqliteConversationRepo;
use crate::app_state::AppState;
use crate::domain::conversation::{Conversation, ConversationTurn};
use crate::domain::types::{ConversationState, TurnRole};
use crate::error::AppError;
use crate::ports::repos::conversation_repo::ConversationRepository;

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
/// Simplified version: receives a user message, ensures an active conversation
/// exists, saves the turn, and returns a message_id. The full runtime streaming
/// will be wired later.
pub async fn send_message(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ConversationMessageRequest>,
) -> Result<Json<ConversationMessageResponse>, AppError> {
    if body.content.is_empty() {
        return Err(AppError::Validation("content must not be empty".into()));
    }

    let repo = SqliteConversationRepo::new(state.db.clone());

    // Get or create an active conversation
    let conversation = match repo.get_pending_or_active().await? {
        Some(mut conv) => {
            if conv.is_pending() {
                conv.activate().map_err(|e| AppError::Internal(e))?;
                repo.update_state(conv.id, ConversationState::Active, None)
                    .await?;
            }
            conv
        }
        None => {
            let conv = Conversation::new_active();
            repo.save(&conv).await?
        }
    };

    let turn = ConversationTurn::new(conversation.id, TurnRole::User, body.content);
    let saved_turn = repo.add_turn(&turn).await?;

    tracing::info!(
        message_id = %saved_turn.id,
        conversation_id = %conversation.id,
        "api.message_accepted",
    );

    Ok(Json(ConversationMessageResponse {
        message_id: saved_turn.id.to_string(),
    }))
}
