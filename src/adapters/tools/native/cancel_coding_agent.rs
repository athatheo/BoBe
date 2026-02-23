use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use super::base::NativeTool;
use crate::error::AppError;
use crate::ports::repos::agent_job_repo::AgentJobRepository;
use crate::ports::tools::{ToolCategory, ToolExecutionContext};

pub struct CancelCodingAgentTool {
    agent_job_repo: Arc<dyn AgentJobRepository>,
}

impl CancelCodingAgentTool {
    pub fn new(agent_job_repo: Arc<dyn AgentJobRepository>) -> Self {
        Self { agent_job_repo }
    }
}

#[async_trait]
impl NativeTool for CancelCodingAgentTool {
    fn name(&self) -> &str {
        "cancel_coding_agent"
    }

    fn description(&self) -> &str {
        "Cancel a running coding agent job."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "job_id": {
                    "type": "string",
                    "description": "UUID of the job to cancel"
                }
            },
            "required": ["job_id"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::System
    }

    async fn execute(
        &self,
        arguments: HashMap<String, Value>,
        _context: Option<&ToolExecutionContext>,
    ) -> Result<String, AppError> {
        let job_id_str = arguments
            .get("job_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Validation("'job_id' is required".into()))?;

        let job_id = Uuid::parse_str(job_id_str)
            .map_err(|_| AppError::Validation(format!("Invalid UUID: {job_id_str}")))?;

        let job = self
            .agent_job_repo
            .get_by_id(job_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Job {job_id} not found")))?;

        if job.is_terminal() {
            return Ok(format!(
                "Job {} is already in terminal state: {}",
                job_id, job.status
            ));
        }

        // Send SIGTERM to the process if it has a PID
        if let Some(pid) = job.pid {
            let _ = std::process::Command::new("kill")
                .arg(pid.to_string())
                .status();
        }

        // Update job to cancelled status
        let mut cancelled = job;
        cancelled.mark_cancelled(Some("Cancelled by user".into()));
        self.agent_job_repo.save(&cancelled).await?;

        Ok(format!("Job {} cancelled.", job_id))
    }
}
