mod conversation_repo;
mod cooldown_repo;
mod soul_repo;
mod user_profile_repo;

pub(crate) mod seeding;

pub(crate) use conversation_repo::SqliteConversationRepo;
pub(crate) use cooldown_repo::SqliteCooldownRepo;
pub(crate) use soul_repo::SqliteSoulRepo;
pub(crate) use user_profile_repo::SqliteUserProfileRepo;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::error::AppError;
use crate::models::conversation::{Conversation, ConversationTurn};
use crate::models::cooldown::CooldownInfo;
use crate::models::ids::{ConversationId, SoulId, UserProfileId};
use crate::models::soul::Soul;
use crate::models::types::{ConversationState, TurnRole};
use crate::models::user_profile::UserProfile;

#[async_trait]
pub(crate) trait ConversationRepository: Send + Sync {
    async fn save(&self, conversation: &Conversation) -> Result<Conversation, AppError>;
    async fn get_by_id(&self, id: ConversationId) -> Result<Option<Conversation>, AppError>;
    async fn get_pending_or_active(&self) -> Result<Option<Conversation>, AppError>;
    async fn get_last_closed(&self) -> Result<Option<Conversation>, AppError>;
    async fn update_state(
        &self,
        id: ConversationId,
        state: ConversationState,
        summary: Option<String>,
    ) -> Result<Option<Conversation>, AppError>;
    async fn add_turn(&self, turn: &ConversationTurn) -> Result<ConversationTurn, AppError>;
    async fn update_turn_content(
        &self,
        turn_id: crate::models::ids::ConversationTurnId,
        content: &str,
    ) -> Result<Option<ConversationTurn>, AppError>;
    async fn get_turns(
        &self,
        conversation_id: ConversationId,
        limit: i64,
    ) -> Result<Vec<ConversationTurn>, AppError>;
    async fn get_recent_turns_by_role(
        &self,
        role: TurnRole,
        limit: i64,
    ) -> Result<Vec<String>, AppError>;
}

#[async_trait]
pub(crate) trait SoulRepository: Send + Sync {
    async fn save(&self, soul: &Soul) -> Result<Soul, AppError>;
    async fn get_by_id(&self, id: SoulId) -> Result<Option<Soul>, AppError>;
    async fn get_by_name(&self, name: &str) -> Result<Option<Soul>, AppError>;
    async fn get_all(&self) -> Result<Vec<Soul>, AppError>;
    async fn find_enabled(&self) -> Result<Vec<Soul>, AppError>;
    async fn update(
        &self,
        id: SoulId,
        content: Option<&str>,
        enabled: Option<bool>,
        is_default: Option<bool>,
        name: Option<&str>,
    ) -> Result<Option<Soul>, AppError>;
    async fn delete(&self, id: SoulId) -> Result<bool, AppError>;
}

#[async_trait]
pub(crate) trait UserProfileRepository: Send + Sync {
    async fn save(&self, profile: &UserProfile) -> Result<UserProfile, AppError>;
    async fn get_by_id(&self, id: UserProfileId) -> Result<Option<UserProfile>, AppError>;
    async fn get_by_name(&self, name: &str) -> Result<Option<UserProfile>, AppError>;
    async fn find_enabled(&self) -> Result<Vec<UserProfile>, AppError>;
    async fn get_all(&self) -> Result<Vec<UserProfile>, AppError>;
    async fn update(
        &self,
        id: UserProfileId,
        content: Option<&str>,
        enabled: Option<bool>,
    ) -> Result<Option<UserProfile>, AppError>;
    async fn delete(&self, id: UserProfileId) -> Result<bool, AppError>;
}

#[async_trait]
pub(crate) trait CooldownRepository: Send + Sync {
    fn check_cooldown(&self, base_minutes: i64, extended_minutes: i64) -> Option<CooldownInfo>;
    async fn load_or_create(&self) -> Result<(), AppError>;
    async fn update_last_engagement(&self, timestamp: DateTime<Utc>) -> Result<(), AppError>;
    async fn update_last_user_response(&self, timestamp: DateTime<Utc>) -> Result<(), AppError>;
}
