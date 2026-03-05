// ─── SQLite repository implementations ──────────────────────────────────────

mod agent_job_repo;
mod conversation_repo;
mod cooldown_repo;
mod goal_plan_repo;
mod goal_repo;
mod learning_state_repo;
mod memory_repo;
mod observation_repo;
mod soul_repo;
mod user_profile_repo;

pub(crate) mod seeding;

pub(crate) use agent_job_repo::SqliteAgentJobRepo;
pub(crate) use conversation_repo::SqliteConversationRepo;
pub(crate) use cooldown_repo::SqliteCooldownRepo;
pub(crate) use goal_plan_repo::SqliteGoalPlanRepo;
pub(crate) use goal_repo::SqliteGoalRepo;
pub(crate) use learning_state_repo::SqliteLearningStateRepo;
pub(crate) use memory_repo::SqliteMemoryRepo;
pub(crate) use observation_repo::SqliteObservationRepo;
pub(crate) use soul_repo::SqliteSoulRepo;
pub(crate) use user_profile_repo::SqliteUserProfileRepo;

// ─── Repository trait definitions ───────────────────────────────────────────

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::error::AppError;
use crate::models::agent_job::AgentJob;
use crate::models::conversation::{Conversation, ConversationTurn};
use crate::models::cooldown::CooldownInfo;
use crate::models::goal::Goal;
use crate::models::goal_plan::{GoalPlan, GoalPlanStep};
use crate::models::ids::{
    AgentJobId, ConversationId, GoalId, GoalPlanId, GoalPlanStepId, MemoryId, ObservationId,
    SoulId, UserProfileId,
};
use crate::models::learning_state::LearningState;
use crate::models::memory::Memory;
use crate::models::observation::Observation;
use crate::models::soul::Soul;
use crate::models::types::{
    AgentJobStatus, ConversationState, GoalPlanStatus, GoalPlanStepStatus, GoalPriority,
    GoalSource, GoalStatus, MemorySource, MemoryType, TurnRole,
};
use crate::models::user_profile::UserProfile;

#[async_trait]
#[allow(dead_code)]
pub(crate) trait ConversationRepository: Send + Sync {
    async fn save(&self, _conversation: &Conversation) -> Result<Conversation, AppError> {
        unimplemented!("ConversationRepository::save")
    }
    async fn get_by_id(&self, _id: ConversationId) -> Result<Option<Conversation>, AppError> {
        unimplemented!("ConversationRepository::get_by_id")
    }
    async fn get_pending_or_active(&self) -> Result<Option<Conversation>, AppError> {
        unimplemented!("ConversationRepository::get_pending_or_active")
    }
    async fn find_closed_since(
        &self,
        _since: Option<DateTime<Utc>>,
    ) -> Result<Vec<Conversation>, AppError> {
        unimplemented!("ConversationRepository::find_closed_since")
    }
    async fn get_last_closed(&self) -> Result<Option<Conversation>, AppError> {
        unimplemented!("ConversationRepository::get_last_closed")
    }
    async fn update_state(
        &self,
        _id: ConversationId,
        _state: ConversationState,
        _summary: Option<String>,
    ) -> Result<Option<Conversation>, AppError> {
        unimplemented!("ConversationRepository::update_state")
    }
    async fn add_turn(&self, _turn: &ConversationTurn) -> Result<ConversationTurn, AppError> {
        unimplemented!("ConversationRepository::add_turn")
    }
    async fn get_turns(
        &self,
        _conversation_id: ConversationId,
        _limit: i64,
    ) -> Result<Vec<ConversationTurn>, AppError> {
        unimplemented!("ConversationRepository::get_turns")
    }
    async fn get_recent_turns_by_role(
        &self,
        _role: TurnRole,
        _limit: i64,
    ) -> Result<Vec<String>, AppError> {
        unimplemented!("ConversationRepository::get_recent_turns_by_role")
    }
    async fn delete(&self, _id: ConversationId) -> Result<bool, AppError> {
        unimplemented!("ConversationRepository::delete")
    }
}

#[async_trait]
#[allow(dead_code)]
pub(crate) trait MemoryRepository: Send + Sync {
    async fn save(&self, _memory: &Memory) -> Result<Memory, AppError> {
        unimplemented!("MemoryRepository::save")
    }
    async fn get_by_id(&self, _id: MemoryId) -> Result<Option<Memory>, AppError> {
        unimplemented!("MemoryRepository::get_by_id")
    }
    async fn find_by_type(
        &self,
        _memory_type: MemoryType,
        _enabled_only: bool,
        _since: Option<DateTime<Utc>>,
    ) -> Result<Vec<Memory>, AppError> {
        unimplemented!("MemoryRepository::find_by_type")
    }
    async fn find_enabled(&self, _limit: Option<i64>) -> Result<Vec<Memory>, AppError> {
        unimplemented!("MemoryRepository::find_enabled")
    }
    async fn find_similar(
        &self,
        _embedding: &[f32],
        _limit: i64,
        _enabled_only: bool,
        _min_score: f64,
    ) -> Result<Vec<(Memory, f64)>, AppError> {
        unimplemented!("MemoryRepository::find_similar")
    }
    async fn find_all(
        &self,
        _memory_type: Option<MemoryType>,
        _category: Option<&str>,
        _source: Option<MemorySource>,
        _enabled_only: bool,
        _limit: i64,
        _offset: i64,
    ) -> Result<(Vec<Memory>, i64), AppError> {
        unimplemented!("MemoryRepository::find_all")
    }
    async fn update(
        &self,
        _id: MemoryId,
        _content: Option<&str>,
        _enabled: Option<bool>,
        _category: Option<&str>,
    ) -> Result<Option<Memory>, AppError> {
        unimplemented!("MemoryRepository::update")
    }
    async fn delete_by_criteria(
        &self,
        _memory_type: MemoryType,
        _older_than: DateTime<Utc>,
    ) -> Result<i64, AppError> {
        unimplemented!("MemoryRepository::delete_by_criteria")
    }
    async fn delete(&self, _id: MemoryId) -> Result<bool, AppError> {
        unimplemented!("MemoryRepository::delete")
    }
    async fn find_null_embedding(&self, _limit: i64) -> Result<Vec<Memory>, AppError> {
        unimplemented!("MemoryRepository::find_null_embedding")
    }
    async fn update_embedding(&self, _id: MemoryId, _embedding: &[f32]) -> Result<(), AppError> {
        unimplemented!("MemoryRepository::update_embedding")
    }
}

#[async_trait]
#[allow(dead_code)]
pub(crate) trait GoalRepository: Send + Sync {
    async fn save(&self, _goal: &Goal) -> Result<Goal, AppError> {
        unimplemented!("GoalRepository::save")
    }
    async fn get_by_id(&self, _id: GoalId) -> Result<Option<Goal>, AppError> {
        unimplemented!("GoalRepository::get_by_id")
    }
    async fn find_by_status(
        &self,
        _status: GoalStatus,
        _enabled_only: bool,
    ) -> Result<Vec<Goal>, AppError> {
        unimplemented!("GoalRepository::find_by_status")
    }
    async fn find_active(&self, _enabled_only: bool) -> Result<Vec<Goal>, AppError> {
        unimplemented!("GoalRepository::find_active")
    }
    async fn find_enabled(&self) -> Result<Vec<Goal>, AppError> {
        unimplemented!("GoalRepository::find_enabled")
    }
    async fn find_similar(
        &self,
        _embedding: &[f32],
        _limit: i64,
        _enabled_only: bool,
    ) -> Result<Vec<(Goal, f64)>, AppError> {
        unimplemented!("GoalRepository::find_similar")
    }
    async fn update_status(
        &self,
        _id: GoalId,
        _status: Option<GoalStatus>,
        _enabled: Option<bool>,
    ) -> Result<Option<Goal>, AppError> {
        unimplemented!("GoalRepository::update_status")
    }
    async fn update_fields(
        &self,
        _id: GoalId,
        _content: Option<&str>,
        _status: Option<GoalStatus>,
        _priority: Option<GoalPriority>,
        _source: Option<GoalSource>,
        _enabled: Option<bool>,
    ) -> Result<Option<Goal>, AppError> {
        unimplemented!("GoalRepository::update_fields")
    }
    async fn delete(&self, _id: GoalId) -> Result<bool, AppError> {
        unimplemented!("GoalRepository::delete")
    }
    async fn delete_stale_goals(
        &self,
        _statuses: &[GoalStatus],
        _older_than: DateTime<Utc>,
    ) -> Result<u64, AppError> {
        unimplemented!("GoalRepository::delete_stale_goals")
    }
    async fn get_all(&self, _include_archived: bool) -> Result<Vec<Goal>, AppError> {
        unimplemented!("GoalRepository::get_all")
    }
    async fn find_null_embedding(&self, _limit: i64) -> Result<Vec<Goal>, AppError> {
        unimplemented!("GoalRepository::find_null_embedding")
    }
    async fn update_embedding(&self, _id: GoalId, _embedding: &[f32]) -> Result<(), AppError> {
        unimplemented!("GoalRepository::update_embedding")
    }
    async fn bulk_update_status(
        &self,
        _goal_ids: &[GoalId],
        _status: GoalStatus,
    ) -> Result<u64, AppError> {
        unimplemented!("GoalRepository::bulk_update_status")
    }
}

#[async_trait]
pub(crate) trait ObservationRepository: Send + Sync {
    async fn save(&self, _observation: &Observation) -> Result<Observation, AppError> {
        unimplemented!("ObservationRepository::save")
    }
    async fn get_by_id(&self, _id: ObservationId) -> Result<Option<Observation>, AppError> {
        unimplemented!("ObservationRepository::get_by_id")
    }
    async fn find_recent(&self, _minutes: i64) -> Result<Vec<Observation>, AppError> {
        unimplemented!("ObservationRepository::find_recent")
    }
    async fn find_since(
        &self,
        _since: Option<DateTime<Utc>>,
        _limit: Option<i64>,
    ) -> Result<Vec<Observation>, AppError> {
        unimplemented!("ObservationRepository::find_since")
    }
    async fn find_similar(
        &self,
        _embedding: &[f32],
        _limit: i64,
    ) -> Result<Vec<(Observation, f64)>, AppError> {
        unimplemented!("ObservationRepository::find_similar")
    }
    async fn delete_older_than(&self, _days: i64) -> Result<i64, AppError> {
        unimplemented!("ObservationRepository::delete_older_than")
    }
    async fn delete(&self, _id: ObservationId) -> Result<bool, AppError> {
        unimplemented!("ObservationRepository::delete")
    }
    async fn find_null_embedding(&self, _limit: i64) -> Result<Vec<Observation>, AppError> {
        unimplemented!("ObservationRepository::find_null_embedding")
    }
    async fn update_embedding(
        &self,
        _id: ObservationId,
        _embedding: &[f32],
    ) -> Result<(), AppError> {
        unimplemented!("ObservationRepository::update_embedding")
    }
}

#[async_trait]
#[allow(dead_code)]
pub(crate) trait SoulRepository: Send + Sync {
    async fn save(&self, _soul: &Soul) -> Result<Soul, AppError> {
        unimplemented!("SoulRepository::save")
    }
    async fn get_by_id(&self, _id: SoulId) -> Result<Option<Soul>, AppError> {
        unimplemented!("SoulRepository::get_by_id")
    }
    async fn get_by_name(&self, _name: &str) -> Result<Option<Soul>, AppError> {
        unimplemented!("SoulRepository::get_by_name")
    }
    async fn get_default(&self) -> Result<Option<Soul>, AppError> {
        unimplemented!("SoulRepository::get_default")
    }
    async fn get_all(&self) -> Result<Vec<Soul>, AppError> {
        unimplemented!("SoulRepository::get_all")
    }
    async fn find_enabled(&self) -> Result<Vec<Soul>, AppError> {
        unimplemented!("SoulRepository::find_enabled")
    }
    async fn update(
        &self,
        _id: SoulId,
        _content: Option<&str>,
        _enabled: Option<bool>,
        _is_default: Option<bool>,
        _name: Option<&str>,
    ) -> Result<Option<Soul>, AppError> {
        unimplemented!("SoulRepository::update")
    }
    async fn delete(&self, _id: SoulId) -> Result<bool, AppError> {
        unimplemented!("SoulRepository::delete")
    }
}

#[async_trait]
#[allow(dead_code)]
pub(crate) trait AgentJobRepository: Send + Sync {
    async fn save(&self, _job: &AgentJob) -> Result<AgentJob, AppError> {
        unimplemented!("AgentJobRepository::save")
    }
    async fn get_by_id(&self, _id: AgentJobId) -> Result<Option<AgentJob>, AppError> {
        unimplemented!("AgentJobRepository::get_by_id")
    }
    async fn find_by_status(&self, _status: AgentJobStatus) -> Result<Vec<AgentJob>, AppError> {
        unimplemented!("AgentJobRepository::find_by_status")
    }
    async fn find_unreported_terminal(&self) -> Result<Vec<AgentJob>, AppError> {
        unimplemented!("AgentJobRepository::find_unreported_terminal")
    }
    async fn mark_reported(&self, _id: AgentJobId) -> Result<(), AppError> {
        unimplemented!("AgentJobRepository::mark_reported")
    }
    async fn get_running_count(&self) -> Result<i64, AppError> {
        unimplemented!("AgentJobRepository::get_running_count")
    }
}

#[async_trait]
#[allow(dead_code)]
pub(crate) trait UserProfileRepository: Send + Sync {
    async fn save(&self, _profile: &UserProfile) -> Result<UserProfile, AppError> {
        unimplemented!("UserProfileRepository::save")
    }
    async fn get_by_id(&self, _id: UserProfileId) -> Result<Option<UserProfile>, AppError> {
        unimplemented!("UserProfileRepository::get_by_id")
    }
    async fn get_by_name(&self, _name: &str) -> Result<Option<UserProfile>, AppError> {
        unimplemented!("UserProfileRepository::get_by_name")
    }
    async fn get_default(&self) -> Result<Option<UserProfile>, AppError> {
        unimplemented!("UserProfileRepository::get_default")
    }
    async fn find_enabled(&self) -> Result<Vec<UserProfile>, AppError> {
        unimplemented!("UserProfileRepository::find_enabled")
    }
    async fn get_all(&self) -> Result<Vec<UserProfile>, AppError> {
        unimplemented!("UserProfileRepository::get_all")
    }
    async fn update(
        &self,
        _id: UserProfileId,
        _content: Option<&str>,
        _enabled: Option<bool>,
    ) -> Result<Option<UserProfile>, AppError> {
        unimplemented!("UserProfileRepository::update")
    }
    async fn delete(&self, _id: UserProfileId) -> Result<bool, AppError> {
        unimplemented!("UserProfileRepository::delete")
    }
}

#[async_trait]
pub(crate) trait LearningStateRepository: Send + Sync {
    async fn get_or_create(&self) -> Result<LearningState, AppError> {
        unimplemented!("LearningStateRepository::get_or_create")
    }
    async fn save(&self, _state: &LearningState) -> Result<(), AppError> {
        unimplemented!("LearningStateRepository::save")
    }
}

#[async_trait]
#[allow(dead_code)]
pub(crate) trait CooldownRepository: Send + Sync {
    fn last_engagement(&self) -> Option<DateTime<Utc>> {
        unimplemented!("CooldownRepository::last_engagement")
    }
    fn last_user_response(&self) -> Option<DateTime<Utc>> {
        unimplemented!("CooldownRepository::last_user_response")
    }
    fn check_cooldown(&self, _base_minutes: i64, _extended_minutes: i64) -> Option<CooldownInfo> {
        unimplemented!("CooldownRepository::check_cooldown")
    }
    async fn load_or_create(&self) -> Result<(), AppError> {
        unimplemented!("CooldownRepository::load_or_create")
    }
    async fn update_last_engagement(&self, _timestamp: DateTime<Utc>) -> Result<(), AppError> {
        unimplemented!("CooldownRepository::update_last_engagement")
    }
    async fn update_last_user_response(&self, _timestamp: DateTime<Utc>) -> Result<(), AppError> {
        unimplemented!("CooldownRepository::update_last_user_response")
    }
}

#[async_trait]
pub(crate) trait GoalPlanRepository: Send + Sync {
    async fn create_plan(
        &self,
        _goal_id: GoalId,
        _summary: &str,
        _status: GoalPlanStatus,
    ) -> Result<GoalPlan, AppError> {
        unimplemented!("GoalPlanRepository::create_plan")
    }
    async fn get_plan(&self, _plan_id: GoalPlanId) -> Result<Option<GoalPlan>, AppError> {
        unimplemented!("GoalPlanRepository::get_plan")
    }
    async fn get_plans_for_goal(&self, _goal_id: GoalId) -> Result<Vec<GoalPlan>, AppError> {
        unimplemented!("GoalPlanRepository::get_plans_for_goal")
    }
    async fn get_active_plan_for_goal(
        &self,
        _goal_id: GoalId,
    ) -> Result<Option<GoalPlan>, AppError> {
        unimplemented!("GoalPlanRepository::get_active_plan_for_goal")
    }
    async fn update_plan_status(
        &self,
        _plan_id: GoalPlanId,
        _status: GoalPlanStatus,
        _error: Option<&str>,
    ) -> Result<Option<GoalPlan>, AppError> {
        unimplemented!("GoalPlanRepository::update_plan_status")
    }
    async fn get_pending_approval_plans(&self) -> Result<Vec<GoalPlan>, AppError> {
        unimplemented!("GoalPlanRepository::get_pending_approval_plans")
    }
    async fn get_expired_pending_plans(
        &self,
        _timeout_minutes: i64,
    ) -> Result<Vec<GoalPlan>, AppError> {
        unimplemented!("GoalPlanRepository::get_expired_pending_plans")
    }
    async fn create_step(
        &self,
        _plan_id: GoalPlanId,
        _step_order: i32,
        _content: &str,
    ) -> Result<GoalPlanStep, AppError> {
        unimplemented!("GoalPlanRepository::create_step")
    }
    async fn update_step_status(
        &self,
        _step_id: GoalPlanStepId,
        _status: GoalPlanStepStatus,
        _result: Option<&str>,
        _error: Option<&str>,
    ) -> Result<Option<GoalPlanStep>, AppError> {
        unimplemented!("GoalPlanRepository::update_step_status")
    }
    async fn get_steps_for_plan(
        &self,
        _plan_id: GoalPlanId,
    ) -> Result<Vec<GoalPlanStep>, AppError> {
        unimplemented!("GoalPlanRepository::get_steps_for_plan")
    }
}
