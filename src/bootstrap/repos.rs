//! Repository trait-object construction from a database pool.

use std::sync::Arc;

use sqlx::sqlite::SqlitePool;

use crate::db::{
    AgentJobRepository, ConversationRepository, CooldownRepository, GoalPlanRepository,
    GoalRepository, LearningStateRepository, MemoryRepository, ObservationRepository,
    SoulRepository, UserProfileRepository,
};
use crate::db::{
    SqliteAgentJobRepo, SqliteConversationRepo, SqliteCooldownRepo, SqliteGoalPlanRepo,
    SqliteGoalRepo, SqliteLearningStateRepo, SqliteMemoryRepo, SqliteObservationRepo,
    SqliteSoulRepo, SqliteUserProfileRepo,
};

pub(crate) struct Repositories {
    pub(crate) conversation_repo: Arc<dyn ConversationRepository>,
    pub(crate) memory_repo: Arc<dyn MemoryRepository>,
    pub(crate) goal_repo: Arc<dyn GoalRepository>,
    pub(crate) observation_repo: Arc<dyn ObservationRepository>,
    pub(crate) cooldown_repo: Arc<dyn CooldownRepository>,
    pub(crate) learning_state_repo: Arc<dyn LearningStateRepository>,
    pub(crate) agent_job_repo: Arc<dyn AgentJobRepository>,
    pub(crate) soul_repo: Arc<dyn SoulRepository>,
    pub(crate) user_profile_repo: Arc<dyn UserProfileRepository>,
    pub(crate) goal_plan_repo: Arc<dyn GoalPlanRepository>,
}

impl Repositories {
    pub(crate) fn from_pool(pool: &SqlitePool) -> Self {
        Self {
            conversation_repo: Arc::new(SqliteConversationRepo::new(pool.clone())),
            memory_repo: Arc::new(SqliteMemoryRepo::new(pool.clone())),
            goal_repo: Arc::new(SqliteGoalRepo::new(pool.clone())),
            observation_repo: Arc::new(SqliteObservationRepo::new(pool.clone())),
            cooldown_repo: Arc::new(SqliteCooldownRepo::new(pool.clone())),
            learning_state_repo: Arc::new(SqliteLearningStateRepo::new(pool.clone())),
            agent_job_repo: Arc::new(SqliteAgentJobRepo::new(pool.clone())),
            soul_repo: Arc::new(SqliteSoulRepo::new(pool.clone())),
            user_profile_repo: Arc::new(SqliteUserProfileRepo::new(pool.clone())),
            goal_plan_repo: Arc::new(SqliteGoalPlanRepo::new(pool.clone())),
        }
    }
}
