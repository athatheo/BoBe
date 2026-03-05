use async_trait::async_trait;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;

use super::base::NativeTool;
use crate::error::AppError;
use crate::models::ids::ConversationId;
use crate::services::agent_job_manager::AgentJobManager;
use crate::tools::ToolExecutionContext;

pub(crate) struct LaunchCodingAgentTool {
    manager: Option<Arc<AgentJobManager>>,
}

impl LaunchCodingAgentTool {
    pub(crate) fn new(manager: Option<Arc<AgentJobManager>>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl NativeTool for LaunchCodingAgentTool {
    fn name(&self) -> &str {
        "launch_coding_agent"
    }

    fn description(&self) -> &str {
        "Launch a coding agent to work on a task in the background. Pass the user's request VERBATIM — do NOT rewrite or enhance it."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task": {
                    "type": "string",
                    "description": "The user's request VERBATIM. Do not rewrite."
                },
                "profile": {
                    "type": "string",
                    "description": "Agent profile name (from available profiles)"
                },
                "working_directory": {
                    "type": "string",
                    "description": "Working directory for the agent (optional)"
                }
            },
            "required": ["task", "profile"]
        })
    }

    async fn execute(
        &self,
        arguments: HashMap<String, Value>,
        context: Option<&ToolExecutionContext>,
    ) -> Result<String, AppError> {
        let task = arguments
            .get("task")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Validation("'task' is required".into()))?;

        let profile = arguments
            .get("profile")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Validation("'profile' is required".into()))?;

        let working_dir = arguments
            .get("working_directory")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

        let conversation_id = context
            .and_then(|c| c.conversation_id.as_deref())
            .unwrap_or("unknown");

        let manager = self
            .manager
            .as_ref()
            .ok_or_else(|| AppError::Validation("Coding agents are disabled".into()))?;

        let conversation_id = conversation_id.parse::<ConversationId>().ok();
        let job = manager
            .launch(profile, task, Some(working_dir), conversation_id)
            .await?;

        Ok(format!(
            "Coding agent launched.\nJob ID: {}\nProfile: {}\nTask: {}\nWorking Directory: {}\nStatus: {}\n\nUse check_coding_agent with this job_id to monitor progress.",
            job.id, profile, task, working_dir, job.status,
        ))
    }
}
