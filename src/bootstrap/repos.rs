//! Repository trait-object construction from a database pool.

use std::sync::Arc;

use sqlx::sqlite::SqlitePool;

use crate::db::{
    AgentJobRepository, ConversationRepository, CooldownRepository, GoalPlanRepository,
    GoalRepository, LearningStateRepository, McpConfigRepository, MemoryRepository,
    ObservationRepository, SoulRepository, UserProfileRepository,
};
use crate::db::{
    SqliteAgentJobRepo, SqliteConversationRepo, SqliteCooldownRepo, SqliteGoalPlanRepo,
    SqliteGoalRepo, SqliteLearningStateRepo, SqliteMcpConfigRepo, SqliteMemoryRepo,
    SqliteObservationRepo, SqliteSoulRepo, SqliteUserProfileRepo,
};

/// All domain repository handles, indexed by trait.
pub struct Repositories {
    pub conversation_repo: Arc<dyn ConversationRepository>,
    pub memory_repo: Arc<dyn MemoryRepository>,
    pub goal_repo: Arc<dyn GoalRepository>,
    pub observation_repo: Arc<dyn ObservationRepository>,
    pub cooldown_repo: Arc<dyn CooldownRepository>,
    pub learning_state_repo: Arc<dyn LearningStateRepository>,
    pub agent_job_repo: Arc<dyn AgentJobRepository>,
    pub mcp_config_repo: Arc<dyn McpConfigRepository>,
    pub soul_repo: Arc<dyn SoulRepository>,
    pub user_profile_repo: Arc<dyn UserProfileRepository>,
    pub goal_plan_repo: Arc<dyn GoalPlanRepository>,
}

impl Repositories {
    /// Construct every repository from a single pool handle.
    pub fn from_pool(pool: &SqlitePool) -> Self {
        Self {
            conversation_repo: Arc::new(SqliteConversationRepo::new(pool.clone())),
            memory_repo: Arc::new(SqliteMemoryRepo::new(pool.clone())),
            goal_repo: Arc::new(SqliteGoalRepo::new(pool.clone())),
            observation_repo: Arc::new(SqliteObservationRepo::new(pool.clone())),
            cooldown_repo: Arc::new(SqliteCooldownRepo::new(pool.clone())),
            learning_state_repo: Arc::new(SqliteLearningStateRepo::new(pool.clone())),
            agent_job_repo: Arc::new(SqliteAgentJobRepo::new(pool.clone())),
            mcp_config_repo: Arc::new(SqliteMcpConfigRepo::new(pool.clone())),
            soul_repo: Arc::new(SqliteSoulRepo::new(pool.clone())),
            user_profile_repo: Arc::new(SqliteUserProfileRepo::new(pool.clone())),
            goal_plan_repo: Arc::new(SqliteGoalPlanRepo::new(pool.clone())),
        }
    }
}
