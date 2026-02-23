use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

use super::base::NativeTool;
use crate::error::AppError;
use crate::ports::repos::observation_repo::ObservationRepository;
use crate::ports::tools::{ToolCategory, ToolExecutionContext};

pub struct GetRecentContextTool {
    observation_repo: Arc<dyn ObservationRepository>,
}

impl GetRecentContextTool {
    pub fn new(observation_repo: Arc<dyn ObservationRepository>) -> Self {
        Self { observation_repo }
    }
}

#[async_trait]
impl NativeTool for GetRecentContextTool {
    fn name(&self) -> &str {
        "get_recent_context"
    }

    fn description(&self) -> &str {
        "Get recent screen observations and captured context."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "integer",
                    "description": "Maximum observations to return (default: 5, max: 20)",
                    "default": 5
                }
            },
            "required": []
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Memory
    }

    async fn execute(
        &self,
        arguments: HashMap<String, Value>,
        _context: Option<&ToolExecutionContext>,
    ) -> Result<String, AppError> {
        let limit = arguments
            .get("limit")
            .and_then(|v| v.as_i64())
            .unwrap_or(5)
            .min(20)
            .max(1);

        let observations = self
            .observation_repo
            .find_since(None, Some(limit))
            .await?;

        if observations.is_empty() {
            return Ok("No recent observations available.".into());
        }

        let mut output = format!("{} recent observations:\n\n", observations.len());
        for (i, obs) in observations.iter().enumerate() {
            let preview = if obs.content.len() > 200 {
                format!("{}...", &obs.content[..200])
            } else {
                obs.content.clone()
            };
            output.push_str(&format!(
                "{}. [{}] {} — {}\n   {}\n\n",
                i + 1,
                obs.source,
                obs.category,
                obs.created_at.format("%H:%M:%S"),
                preview,
            ));
        }
        Ok(output)
    }
}
