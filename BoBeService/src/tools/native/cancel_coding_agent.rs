use super::base::NativeTool;
use crate::error::AppError;
use crate::models::ids::AgentJobId;
use crate::services::agent_job_manager::AgentJobManager;
use crate::tools::ToolExecutionContext;
use async_trait::async_trait;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;

pub(crate) struct CancelCodingAgentTool {
    manager: Option<Arc<AgentJobManager>>,
}

impl CancelCodingAgentTool {
    pub(crate) fn new(manager: Option<Arc<AgentJobManager>>) -> Self {
        Self { manager }
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

    async fn execute(
        &self,
        arguments: HashMap<String, Value>,
        _context: Option<&ToolExecutionContext>,
    ) -> Result<String, AppError> {
        let job_id_str = arguments
            .get("job_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Validation("'job_id' is required".into()))?;

        let job_id: AgentJobId = job_id_str
            .parse()
            .map_err(|_| AppError::Validation(format!("Invalid UUID: {job_id_str}")))?;

        let manager = self
            .manager
            .as_ref()
            .ok_or_else(|| AppError::Validation("Coding agents are disabled".into()))?;
        let job = manager.cancel(job_id).await?;

        if job.status.is_terminal() && job.status != crate::models::types::AgentJobStatus::Cancelled
        {
            return Ok(format!(
                "Job {} is already in terminal state: {}",
                job_id, job.status
            ));
        }

        Ok(format!("Job {job_id} cancelled."))
    }
}
