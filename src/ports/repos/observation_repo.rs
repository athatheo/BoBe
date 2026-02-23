use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::observation::Observation;
use crate::error::AppError;

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
}
