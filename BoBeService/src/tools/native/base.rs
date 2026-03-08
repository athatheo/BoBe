use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

use crate::error::AppError;

#[async_trait]
pub(crate) trait NativeTool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;

    async fn execute(
        &self,
        arguments: HashMap<String, Value>,
        context: Option<&crate::tools::ToolExecutionContext>,
    ) -> Result<String, AppError>;
}
