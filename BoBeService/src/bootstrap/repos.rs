use std::sync::Arc;

use sqlx::sqlite::SqlitePool;

use crate::db::{
    SqliteConversationRepo, SqliteCooldownRepo, SqliteSoulRepo, SqliteUserProfileRepo,
};

pub(crate) struct Repositories {
    pub(crate) conversation_repo: Arc<SqliteConversationRepo>,
    pub(crate) cooldown_repo: Arc<SqliteCooldownRepo>,
    pub(crate) soul_repo: Arc<SqliteSoulRepo>,
    pub(crate) user_profile_repo: Arc<SqliteUserProfileRepo>,
}

impl Repositories {
    pub(crate) fn from_pool(pool: &SqlitePool) -> Self {
        Self {
            conversation_repo: Arc::new(SqliteConversationRepo::new(pool.clone())),
            cooldown_repo: Arc::new(SqliteCooldownRepo::new(pool.clone())),
            soul_repo: Arc::new(SqliteSoulRepo::new(pool.clone())),
            user_profile_repo: Arc::new(SqliteUserProfileRepo::new(pool.clone())),
        }
    }
}
