use super::base::NativeTool;
use crate::db::AgentJobRepository;
use crate::error::AppError;
use crate::models::ids::AgentJobId;
use crate::tools::ToolExecutionContext;
use async_trait::async_trait;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fmt::Write;
use std::sync::Arc;

pub(crate) struct CheckCodingAgentTool {
    agent_job_repo: Arc<dyn AgentJobRepository>,
}

impl CheckCodingAgentTool {
    pub(crate) fn new(agent_job_repo: Arc<dyn AgentJobRepository>) -> Self {
        Self { agent_job_repo }
    }
}

#[async_trait]
impl NativeTool for CheckCodingAgentTool {
    fn name(&self) -> &str {
        "check_coding_agent"
    }

    fn description(&self) -> &str {
        "Check the status of a running coding agent job."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "job_id": {
                    "type": "string",
                    "description": "UUID of the job from launch_coding_agent"
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

        let job = self
            .agent_job_repo
            .get_by_id(job_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Job {job_id} not found")))?;

        let runtime = job
            .runtime_seconds()
            .map_or_else(|| "N/A".into(), |s| format!("{s:.1}s"));

        let mut output = format!(
            "Job: {}\nStatus: {}\nProfile: {}\nTask: {}\nRuntime: {}\n",
            job.id, job.status, job.profile_name, job.user_intent, runtime,
        );

        if let Some(summary) = &job.result_summary {
            let _ = write!(output, "\nSummary:\n{summary}\n");
        }
        if let Some(err) = &job.error_message {
            let _ = write!(output, "\nError: {err}\n");
        }
        if let Some(files_json) = &job.files_changed_json {
            let _ = write!(output, "\nFiles changed: {files_json}\n");
        }
        if let Some(cost) = job.cost_usd {
            let _ = writeln!(output, "Cost: ${cost:.4}");
        }

        Ok(output)
    }
}
