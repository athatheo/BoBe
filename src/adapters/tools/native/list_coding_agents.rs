use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

use super::base::NativeTool;
use crate::domain::types::AgentJobStatus;
use crate::error::AppError;
use crate::ports::repos::agent_job_repo::AgentJobRepository;
use crate::ports::tools::{ToolCategory, ToolExecutionContext};

pub struct ListCodingAgentsTool {
    agent_job_repo: Arc<dyn AgentJobRepository>,
}

impl ListCodingAgentsTool {
    pub fn new(agent_job_repo: Arc<dyn AgentJobRepository>) -> Self {
        Self { agent_job_repo }
    }
}

#[async_trait]
impl NativeTool for ListCodingAgentsTool {
    fn name(&self) -> &str {
        "list_coding_agents"
    }

    fn description(&self) -> &str {
        "List active and recent coding agent jobs."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::System
    }

    async fn execute(
        &self,
        _arguments: HashMap<String, Value>,
        _context: Option<&ToolExecutionContext>,
    ) -> Result<String, AppError> {
        let running = self
            .agent_job_repo
            .find_by_status(AgentJobStatus::Running)
            .await?;

        let pending = self
            .agent_job_repo
            .find_by_status(AgentJobStatus::Pending)
            .await?;

        if running.is_empty() && pending.is_empty() {
            return Ok("No active coding agent jobs.".into());
        }

        let mut output = String::new();

        if !running.is_empty() {
            output.push_str(&format!("## Running ({}):\n\n", running.len()));
            for job in &running {
                let runtime = job
                    .runtime_seconds()
                    .map(|s| format!("{s:.0}s"))
                    .unwrap_or_else(|| "N/A".into());
                output.push_str(&format!(
                    "• [{}] {} — {}\n  Profile: {} | Runtime: {}\n\n",
                    job.id, job.user_intent, job.status, job.profile_name, runtime,
                ));
            }
        }

        if !pending.is_empty() {
            output.push_str(&format!("## Pending ({}):\n\n", pending.len()));
            for job in &pending {
                output.push_str(&format!(
                    "• [{}] {} — Profile: {}\n\n",
                    job.id, job.user_intent, job.profile_name,
                ));
            }
        }

        Ok(output)
    }
}
