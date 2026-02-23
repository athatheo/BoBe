use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::domain::cooldown::CooldownInfo;
use crate::error::AppError;

#[async_trait]
pub trait CooldownRepository: Send + Sync {
    fn last_engagement(&self) -> Option<DateTime<Utc>>;
    fn last_user_response(&self) -> Option<DateTime<Utc>>;
    fn check_cooldown(&self, base_minutes: i64, extended_minutes: i64) -> Option<CooldownInfo>;
    async fn load_or_create(&self) -> Result<(), AppError>;
    async fn update_last_engagement(&self, timestamp: DateTime<Utc>) -> Result<(), AppError>;
    async fn update_last_user_response(&self, timestamp: DateTime<Utc>) -> Result<(), AppError>;
}
