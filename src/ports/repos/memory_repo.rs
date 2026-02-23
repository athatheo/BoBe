use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::memory::Memory;
use crate::domain::types::MemoryType;
use crate::error::AppError;

#[async_trait]
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
}
