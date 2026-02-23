use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::goal::Goal;
use crate::domain::types::{GoalPriority, GoalSource, GoalStatus};
use crate::error::AppError;

#[async_trait]
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
