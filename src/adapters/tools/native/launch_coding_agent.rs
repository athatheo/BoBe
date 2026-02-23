use async_trait::async_trait;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;

use super::base::NativeTool;
use crate::error::AppError;
use crate::ports::repos::agent_job_repo::AgentJobRepository;
use crate::ports::tools::{ToolCategory, ToolExecutionContext};

pub struct LaunchCodingAgentTool {
    agent_job_repo: Arc<dyn AgentJobRepository>,
}

impl LaunchCodingAgentTool {
    pub fn new(agent_job_repo: Arc<dyn AgentJobRepository>) -> Self {
        Self { agent_job_repo }
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

    fn category(&self) -> ToolCategory {
        ToolCategory::System
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

        // Create the agent job record
        let mut job = crate::domain::agent_job::AgentJob::new(
            profile.to_owned(),
            profile.to_owned(), // command comes from profile resolution
            task.to_owned(),
            working_dir.to_owned(),
        );
        if let Ok(cid) = uuid::Uuid::parse_str(conversation_id) {
            job.conversation_id = Some(cid);
        }

        let saved = self.agent_job_repo.save(&job).await?;

        Ok(format!(
            "Coding agent launched.\nJob ID: {}\nProfile: {}\nTask: {}\nWorking Directory: {}\n\nUse check_coding_agent with this job_id to monitor progress.",
            saved.id, profile, task, working_dir,
        ))
    }
}
