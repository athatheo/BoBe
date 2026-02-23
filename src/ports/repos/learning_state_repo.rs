use async_trait::async_trait;

use crate::domain::learning_state::LearningState;
use crate::error::AppError;

#[async_trait]
pub trait LearningStateRepository: Send + Sync {
    async fn get_or_create(&self) -> Result<LearningState, AppError>;
    async fn save(&self, state: &LearningState) -> Result<(), AppError>;
}
