// ─── SQLite repository implementations ──────────────────────────────────────

mod conversation_repo;
mod memory_repo;
mod goal_repo;
mod observation_repo;
mod soul_repo;
mod agent_job_repo;
mod user_profile_repo;
mod learning_state_repo;
mod cooldown_repo;
mod mcp_config_repo;
mod goal_plan_repo;

pub mod seeding;

pub use conversation_repo::SqliteConversationRepo;
pub use memory_repo::SqliteMemoryRepo;
pub use goal_repo::SqliteGoalRepo;
pub use observation_repo::SqliteObservationRepo;
pub use soul_repo::SqliteSoulRepo;
pub use agent_job_repo::SqliteAgentJobRepo;
pub use user_profile_repo::SqliteUserProfileRepo;
pub use learning_state_repo::SqliteLearningStateRepo;
pub use cooldown_repo::SqliteCooldownRepo;
pub use mcp_config_repo::SqliteMcpConfigRepo;
pub use goal_plan_repo::SqliteGoalPlanRepo;

// ─── Repository trait definitions ───────────────────────────────────────────

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::agent_job::AgentJob;
use crate::models::conversation::{Conversation, ConversationTurn};
use crate::models::cooldown::CooldownInfo;
use crate::models::goal::Goal;
use crate::models::goal_plan::{GoalPlan, GoalPlanStep};
use crate::models::learning_state::LearningState;
use crate::models::mcp_server_config::McpServerConfig;
use crate::models::memory::Memory;
use crate::models::observation::Observation;
use crate::models::soul::Soul;
use crate::models::types::{
    AgentJobStatus, ConversationState, GoalPlanStatus, GoalPlanStepStatus, GoalPriority,
    GoalSource, GoalStatus, MemoryType, TurnRole,
};
use crate::models::user_profile::UserProfile;

#[async_trait]
#[allow(dead_code)]
pub trait ConversationRepository: Send + Sync {
    async fn save(&self, conversation: &Conversation) -> Result<Conversation, AppError>;
    async fn get_by_id(&self, id: Uuid) -> Result<Option<Conversation>, AppError>;
    async fn get_active(&self) -> Result<Option<Conversation>, AppError>;
    async fn get_pending_or_active(&self) -> Result<Option<Conversation>, AppError>;
    async fn find_by_state(
        &self,
        state: ConversationState,
        limit: i64,
    ) -> Result<Vec<Conversation>, AppError>;
    async fn find_recent(&self, limit: i64) -> Result<Vec<Conversation>, AppError>;
    async fn find_closed_since(
        &self,
        since: Option<DateTime<Utc>>,
    ) -> Result<Vec<Conversation>, AppError>;
    async fn get_last_closed(&self) -> Result<Option<Conversation>, AppError>;
    async fn update_state(
        &self,
        id: Uuid,
        state: ConversationState,
        summary: Option<String>,
    ) -> Result<Option<Conversation>, AppError>;
    async fn add_turn(&self, turn: &ConversationTurn) -> Result<ConversationTurn, AppError>;
    async fn get_turns(
        &self,
        conversation_id: Uuid,
        limit: i64,
    ) -> Result<Vec<ConversationTurn>, AppError>;
    async fn get_recent_turns(&self, limit: i64) -> Result<Vec<ConversationTurn>, AppError>;
    async fn get_recent_turns_by_role(
        &self,
        role: TurnRole,
        limit: i64,
    ) -> Result<Vec<String>, AppError>;
    async fn delete(&self, id: Uuid) -> Result<bool, AppError>;
}

#[async_trait]
#[allow(dead_code)]
pub trait MemoryRepository: Send + Sync {
    async fn save(&self, memory: &Memory) -> Result<Memory, AppError>;
    async fn get_by_id(&self, id: Uuid) -> Result<Option<Memory>, AppError>;
    async fn find_by_type(
        &self,
        memory_type: MemoryType,
        enabled_only: bool,
        since: Option<DateTime<Utc>>,
    ) -> Result<Vec<Memory>, AppError>;
    async fn find_enabled(&self, limit: Option<i64>) -> Result<Vec<Memory>, AppError>;
    async fn find_similar(
        &self,
        embedding: &[f32],
        limit: i64,
        enabled_only: bool,
        min_score: f64,
    ) -> Result<Vec<(Memory, f64)>, AppError>;
    async fn find_all(
        &self,
        memory_type: Option<&str>,
        category: Option<&str>,
        source: Option<&str>,
        enabled_only: bool,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<Memory>, i64), AppError>;
    async fn update(
        &self,
        id: Uuid,
        content: Option<&str>,
        enabled: Option<bool>,
        category: Option<&str>,
    ) -> Result<Option<Memory>, AppError>;
    async fn delete_by_criteria(
        &self,
        memory_type: MemoryType,
        older_than: DateTime<Utc>,
    ) -> Result<i64, AppError>;
    async fn delete(&self, id: Uuid) -> Result<bool, AppError>;
    async fn find_null_embedding(&self, limit: i64) -> Result<Vec<Memory>, AppError>;
    async fn update_embedding(&self, id: Uuid, embedding: &[f32]) -> Result<(), AppError>;
}

#[async_trait]
#[allow(dead_code)]
pub trait GoalRepository: Send + Sync {
    async fn save(&self, goal: &Goal) -> Result<Goal, AppError>;
    async fn get_by_id(&self, id: Uuid) -> Result<Option<Goal>, AppError>;
    async fn find_by_status(
        &self,
        status: GoalStatus,
        enabled_only: bool,
    ) -> Result<Vec<Goal>, AppError>;
    async fn find_active(&self, enabled_only: bool) -> Result<Vec<Goal>, AppError>;
    async fn find_enabled(&self) -> Result<Vec<Goal>, AppError>;
    async fn find_similar(
        &self,
        embedding: &[f32],
        limit: i64,
        enabled_only: bool,
    ) -> Result<Vec<(Goal, f64)>, AppError>;
    async fn update_status(
        &self,
        id: Uuid,
        status: Option<GoalStatus>,
        enabled: Option<bool>,
    ) -> Result<Option<Goal>, AppError>;
    async fn update_fields(
        &self,
        id: Uuid,
        content: Option<&str>,
        status: Option<GoalStatus>,
        priority: Option<GoalPriority>,
        source: Option<GoalSource>,
        enabled: Option<bool>,
    ) -> Result<Option<Goal>, AppError>;
    async fn delete(&self, id: Uuid) -> Result<bool, AppError>;
    /// Delete goals with given statuses that were updated before the cutoff.
    async fn delete_stale_goals(
        &self,
        statuses: &[GoalStatus],
        older_than: DateTime<Utc>,
    ) -> Result<u64, AppError>;
    async fn find_by_content(&self, content: &str) -> Result<Option<Goal>, AppError>;
    async fn get_all(&self, include_archived: bool) -> Result<Vec<Goal>, AppError>;
    async fn find_null_embedding(&self, limit: i64) -> Result<Vec<Goal>, AppError>;
    async fn update_embedding(&self, id: Uuid, embedding: &[f32]) -> Result<(), AppError>;
    /// Bulk update status for multiple goals. Returns count of updated rows.
    async fn bulk_update_status(
        &self,
        goal_ids: &[Uuid],
        status: GoalStatus,
    ) -> Result<u64, AppError>;
}

#[async_trait]
pub trait ObservationRepository: Send + Sync {
    async fn save(&self, observation: &Observation) -> Result<Observation, AppError>;
    async fn get_by_id(&self, id: Uuid) -> Result<Option<Observation>, AppError>;
    async fn find_recent(&self, minutes: i64) -> Result<Vec<Observation>, AppError>;
    async fn find_since(
        &self,
        since: Option<DateTime<Utc>>,
        limit: Option<i64>,
    ) -> Result<Vec<Observation>, AppError>;
    async fn find_similar(
        &self,
        embedding: &[f32],
        limit: i64,
    ) -> Result<Vec<(Observation, f64)>, AppError>;
    async fn delete_older_than(&self, days: i64) -> Result<i64, AppError>;
    async fn delete(&self, id: Uuid) -> Result<bool, AppError>;
    async fn find_null_embedding(&self, limit: i64) -> Result<Vec<Observation>, AppError>;
    async fn update_embedding(&self, id: Uuid, embedding: &[f32]) -> Result<(), AppError>;
}

#[async_trait]
#[allow(dead_code)]
pub trait SoulRepository: Send + Sync {
    async fn save(&self, soul: &Soul) -> Result<Soul, AppError>;
    async fn get_by_id(&self, id: Uuid) -> Result<Option<Soul>, AppError>;
    async fn get_by_name(&self, name: &str) -> Result<Option<Soul>, AppError>;
    async fn get_default(&self) -> Result<Option<Soul>, AppError>;
    async fn get_all(&self) -> Result<Vec<Soul>, AppError>;
    async fn find_enabled(&self) -> Result<Vec<Soul>, AppError>;
    async fn update(
        &self,
        id: Uuid,
        content: Option<&str>,
        enabled: Option<bool>,
        is_default: Option<bool>,
        name: Option<&str>,
    ) -> Result<Option<Soul>, AppError>;
    async fn delete(&self, id: Uuid) -> Result<bool, AppError>;
}

#[async_trait]
#[allow(dead_code)]
pub trait AgentJobRepository: Send + Sync {
    async fn save(&self, job: &AgentJob) -> Result<AgentJob, AppError>;
    async fn get_by_id(&self, id: Uuid) -> Result<Option<AgentJob>, AppError>;
    async fn find_by_status(&self, status: AgentJobStatus) -> Result<Vec<AgentJob>, AppError>;
    async fn find_unreported_terminal(&self) -> Result<Vec<AgentJob>, AppError>;
    async fn mark_reported(&self, id: Uuid) -> Result<(), AppError>;
    async fn get_running_count(&self) -> Result<i64, AppError>;
}

#[async_trait]
#[allow(dead_code)]
pub trait UserProfileRepository: Send + Sync {
    async fn save(&self, profile: &UserProfile) -> Result<UserProfile, AppError>;
    async fn get_by_id(&self, id: Uuid) -> Result<Option<UserProfile>, AppError>;
    async fn get_by_name(&self, name: &str) -> Result<Option<UserProfile>, AppError>;
    async fn get_default(&self) -> Result<Option<UserProfile>, AppError>;
    async fn find_enabled(&self) -> Result<Vec<UserProfile>, AppError>;
    async fn get_all(&self) -> Result<Vec<UserProfile>, AppError>;
    async fn update(
        &self,
        id: Uuid,
        content: Option<&str>,
        enabled: Option<bool>,
    ) -> Result<Option<UserProfile>, AppError>;
    async fn delete(&self, id: Uuid) -> Result<bool, AppError>;
}

#[async_trait]
pub trait LearningStateRepository: Send + Sync {
    async fn get_or_create(&self) -> Result<LearningState, AppError>;
    async fn save(&self, state: &LearningState) -> Result<(), AppError>;
}

#[async_trait]
#[allow(dead_code)]
pub trait CooldownRepository: Send + Sync {
    fn last_engagement(&self) -> Option<DateTime<Utc>>;
    fn last_user_response(&self) -> Option<DateTime<Utc>>;
    fn check_cooldown(&self, base_minutes: i64, extended_minutes: i64) -> Option<CooldownInfo>;
    async fn load_or_create(&self) -> Result<(), AppError>;
    async fn update_last_engagement(&self, timestamp: DateTime<Utc>) -> Result<(), AppError>;
    async fn update_last_user_response(&self, timestamp: DateTime<Utc>) -> Result<(), AppError>;
}

#[async_trait]
pub trait McpConfigRepository: Send + Sync {
    async fn save(&self, config: &McpServerConfig) -> Result<McpServerConfig, AppError>;
    async fn get_by_id(&self, id: Uuid) -> Result<Option<McpServerConfig>, AppError>;
    async fn get_by_name(&self, name: &str) -> Result<Option<McpServerConfig>, AppError>;
    async fn get_all(&self) -> Result<Vec<McpServerConfig>, AppError>;
    async fn find_enabled(&self) -> Result<Vec<McpServerConfig>, AppError>;
    async fn update(
        &self,
        id: Uuid,
        command: Option<&str>,
        args: Option<&str>,
        env: Option<&str>,
        enabled: Option<bool>,
        timeout_seconds: Option<f64>,
        excluded_tools: Option<&str>,
    ) -> Result<Option<McpServerConfig>, AppError>;
    async fn delete(&self, id: Uuid) -> Result<bool, AppError>;
}

#[async_trait]
pub trait GoalPlanRepository: Send + Sync {
    async fn create_plan(
        &self,
        goal_id: Uuid,
        summary: &str,
        status: GoalPlanStatus,
    ) -> Result<GoalPlan, AppError>;
    async fn get_plan(&self, plan_id: Uuid) -> Result<Option<GoalPlan>, AppError>;
    async fn get_plans_for_goal(&self, goal_id: Uuid) -> Result<Vec<GoalPlan>, AppError>;
    async fn get_active_plan_for_goal(
        &self,
        goal_id: Uuid,
    ) -> Result<Option<GoalPlan>, AppError>;
    async fn update_plan_status(
        &self,
        plan_id: Uuid,
        status: GoalPlanStatus,
        error: Option<&str>,
    ) -> Result<Option<GoalPlan>, AppError>;
    async fn get_pending_approval_plans(&self) -> Result<Vec<GoalPlan>, AppError>;
    async fn get_expired_pending_plans(
        &self,
        timeout_minutes: i64,
    ) -> Result<Vec<GoalPlan>, AppError>;
    async fn create_step(
        &self,
        plan_id: Uuid,
        step_order: i32,
        content: &str,
    ) -> Result<GoalPlanStep, AppError>;
    async fn update_step_status(
        &self,
        step_id: Uuid,
        status: GoalPlanStepStatus,
        result: Option<&str>,
        error: Option<&str>,
    ) -> Result<Option<GoalPlanStep>, AppError>;
    async fn get_steps_for_plan(&self, plan_id: Uuid) -> Result<Vec<GoalPlanStep>, AppError>;
}
