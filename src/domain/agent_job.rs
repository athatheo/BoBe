use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::types::AgentJobStatus;

/// Tracks a coding agent subprocess from launch to completion.
///
/// State Machine:
///   PENDING → RUNNING → COMPLETED | FAILED | CANCELLED
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct AgentJob {
    pub id: Uuid,
    pub profile_name: String,
    pub command: String,
    pub user_intent: String,
    pub status: AgentJobStatus,
    pub working_directory: String,
    pub conversation_id: Option<Uuid>,
    pub pid: Option<i64>,
    pub exit_code: Option<i32>,
    pub result_summary: Option<String>,
    pub raw_output_path: Option<String>,
    pub error_message: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub cost_usd: Option<f64>,
    pub files_changed_json: Option<String>,
    pub agent_session_id: Option<String>,
    pub continuation_count: i32,
    pub reported: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl AgentJob {
    pub fn new(
        profile_name: String,
        command: String,
        user_intent: String,
        working_directory: String,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            profile_name,
            command,
            user_intent,
            status: AgentJobStatus::Pending,
            working_directory,
            conversation_id: None,
            pid: None,
            exit_code: None,
            result_summary: None,
            raw_output_path: None,
            error_message: None,
            started_at: None,
            completed_at: None,
            cost_usd: None,
            files_changed_json: None,
            agent_session_id: None,
            continuation_count: 0,
            reported: false,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn is_terminal(&self) -> bool {
        self.status.is_terminal()
    }

    pub fn runtime_seconds(&self) -> Option<f64> {
        let started = self.started_at?;
        let end = self.completed_at.unwrap_or_else(Utc::now);
        Some((end - started).num_milliseconds() as f64 / 1000.0)
    }

    pub fn mark_running(&mut self, pid: i64) {
        self.status = AgentJobStatus::Running;
        self.pid = Some(pid);
        self.started_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }

    pub fn mark_completed(&mut self, exit_code: i32, summary: Option<String>) {
        self.status = AgentJobStatus::Completed;
        self.exit_code = Some(exit_code);
        self.completed_at = Some(Utc::now());
        self.updated_at = Utc::now();
        if let Some(s) = summary {
            self.result_summary = Some(s);
        }
    }

    pub fn mark_failed(&mut self, error: String, exit_code: Option<i32>) {
        self.status = AgentJobStatus::Failed;
        self.error_message = Some(error);
        self.completed_at = Some(Utc::now());
        self.updated_at = Utc::now();
        if let Some(code) = exit_code {
            self.exit_code = Some(code);
        }
    }

    pub fn mark_cancelled(&mut self, reason: Option<String>) {
        self.status = AgentJobStatus::Cancelled;
        self.completed_at = Some(Utc::now());
        self.updated_at = Utc::now();
        if let Some(r) = reason {
            self.error_message = Some(format!("Cancelled: {r}"));
        }
    }
}
