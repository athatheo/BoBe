use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::soul::Soul;
use crate::error::AppError;

#[async_trait]
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
