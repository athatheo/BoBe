use async_trait::async_trait;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fmt::Write;
use std::sync::Arc;

use super::base::NativeTool;
use crate::db::AgentJobRepository;
use crate::error::AppError;
use crate::models::types::AgentJobStatus;
use crate::tools::ToolExecutionContext;

pub(crate) struct ListCodingAgentsTool {
    agent_job_repo: Arc<dyn AgentJobRepository>,
}

impl ListCodingAgentsTool {
    pub(crate) fn new(agent_job_repo: Arc<dyn AgentJobRepository>) -> Self {
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
            let _ = write!(output, "## Running ({}):\n\n", running.len());
            for job in &running {
                let runtime = job
                    .runtime_seconds()
                    .map_or_else(|| "N/A".into(), |s| format!("{s:.0}s"));
                let _ = write!(
                    output,
                    "• [{}] {} — {}\n  Profile: {} | Runtime: {}\n\n",
                    job.id, job.user_intent, job.status, job.profile_name, runtime,
                );
            }
        }

        if !pending.is_empty() {
            let _ = write!(output, "## Pending ({}):\n\n", pending.len());
            for job in &pending {
                let _ = write!(
                    output,
                    "• [{}] {} — Profile: {}\n\n",
                    job.id, job.user_intent, job.profile_name,
                );
            }
        }

        Ok(output)
    }
}
