use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::agent_job::AgentJob;
use crate::domain::types::AgentJobStatus;
use crate::error::AppError;

#[async_trait]
pub trait AgentJobRepository: Send + Sync {
    async fn save(&self, job: &AgentJob) -> Result<AgentJob, AppError>;
    async fn get_by_id(&self, id: Uuid) -> Result<Option<AgentJob>, AppError>;
    async fn find_by_status(&self, status: AgentJobStatus) -> Result<Vec<AgentJob>, AppError>;
    async fn find_unreported_terminal(&self) -> Result<Vec<AgentJob>, AppError>;
    async fn mark_reported(&self, id: Uuid) -> Result<(), AppError>;
    async fn get_running_count(&self) -> Result<i64, AppError>;
}
