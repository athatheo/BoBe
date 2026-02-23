use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::user_profile::UserProfile;
use crate::error::AppError;

#[async_trait]
pub trait UserProfileRepository: Send + Sync {
    async fn save(&self, profile: &UserProfile) -> Result<UserProfile, AppError>;
    async fn get_by_id(&self, id: Uuid) -> Result<Option<UserProfile>, AppError>;
    async fn get_by_name(&self, name: &str) -> Result<Option<UserProfile>, AppError>;
    async fn get_default(&self) -> Result<Option<UserProfile>, AppError>;
    async fn find_enabled(&self) -> Result<Vec<UserProfile>, AppError>;
    async fn get_all(&self) -> Result<Vec<UserProfile>, AppError>;
    async fn update(
        &self,
        id: Uuid,
        content: Option<&str>,
        enabled: Option<bool>,
    ) -> Result<Option<UserProfile>, AppError>;
    async fn delete(&self, id: Uuid) -> Result<bool, AppError>;
}
