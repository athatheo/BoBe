use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::mcp_server_config::McpServerConfig;
use crate::error::AppError;

#[async_trait]
pub trait McpConfigRepository: Send + Sync {
    async fn save(&self, config: &McpServerConfig) -> Result<McpServerConfig, AppError>;
    async fn get_by_id(&self, id: Uuid) -> Result<Option<McpServerConfig>, AppError>;
    async fn get_by_name(&self, name: &str) -> Result<Option<McpServerConfig>, AppError>;
    async fn get_all(&self) -> Result<Vec<McpServerConfig>, AppError>;
    async fn find_enabled(&self) -> Result<Vec<McpServerConfig>, AppError>;
    async fn update(
        &self,
        id: Uuid,
        command: Option<&str>,
        args: Option<&str>,
        env: Option<&str>,
        enabled: Option<bool>,
        timeout_seconds: Option<f64>,
        excluded_tools: Option<&str>,
    ) -> Result<Option<McpServerConfig>, AppError>;
    async fn delete(&self, id: Uuid) -> Result<bool, AppError>;
}
