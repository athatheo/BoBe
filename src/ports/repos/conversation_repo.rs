use async_trait::async_trait;
use chrono::DateTime;
use chrono::Utc;
use uuid::Uuid;

use crate::domain::conversation::{Conversation, ConversationTurn};
use crate::domain::types::{ConversationState, TurnRole};
use crate::error::AppError;

#[async_trait]
pub trait ConversationRepository: Send + Sync {
    async fn save(&self, conversation: &Conversation) -> Result<Conversation, AppError>;
    async fn get_by_id(&self, id: Uuid) -> Result<Option<Conversation>, AppError>;
    async fn get_active(&self) -> Result<Option<Conversation>, AppError>;
    async fn get_pending_or_active(&self) -> Result<Option<Conversation>, AppError>;
    async fn find_by_state(&self, state: ConversationState, limit: i64) -> Result<Vec<Conversation>, AppError>;
    async fn find_recent(&self, limit: i64) -> Result<Vec<Conversation>, AppError>;
    async fn find_closed_since(&self, since: Option<DateTime<Utc>>) -> Result<Vec<Conversation>, AppError>;
    async fn get_last_closed(&self) -> Result<Option<Conversation>, AppError>;
    async fn update_state(&self, id: Uuid, state: ConversationState, summary: Option<String>) -> Result<Option<Conversation>, AppError>;
    async fn add_turn(&self, turn: &ConversationTurn) -> Result<ConversationTurn, AppError>;
    async fn get_turns(&self, conversation_id: Uuid, limit: i64) -> Result<Vec<ConversationTurn>, AppError>;
    async fn get_recent_turns(&self, limit: i64) -> Result<Vec<ConversationTurn>, AppError>;
    async fn get_recent_turns_by_role(&self, role: TurnRole, limit: i64) -> Result<Vec<String>, AppError>;
    async fn delete(&self, id: Uuid) -> Result<bool, AppError>;
}
