use async_trait::async_trait;
use sqlx::SqlitePool;
use tracing::{debug, info};
use uuid::Uuid;

use crate::db::AgentJobRepository;
use crate::error::AppError;
use crate::models::agent_job::AgentJob;
use crate::models::types::AgentJobStatus;

pub(crate) struct SqliteAgentJobRepo {
    pool: SqlitePool,
}

impl SqliteAgentJobRepo {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AgentJobRepository for SqliteAgentJobRepo {
    async fn save(&self, job: &AgentJob) -> Result<AgentJob, AppError> {
        sqlx::query(
            r"INSERT INTO agent_jobs (id, profile_name, command, user_intent, status, working_directory,
                   conversation_id, pid, exit_code, result_summary, raw_output_path, error_message,
                   started_at, completed_at, cost_usd, files_changed_json, agent_session_id,
                   continuation_count, reported, created_at, updated_at)
               VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21)
               ON CONFLICT(id) DO UPDATE SET
                   status = excluded.status,
                   pid = excluded.pid,
                   exit_code = excluded.exit_code,
                   result_summary = excluded.result_summary,
                   raw_output_path = excluded.raw_output_path,
                   error_message = excluded.error_message,
                   started_at = excluded.started_at,
                   completed_at = excluded.completed_at,
                   cost_usd = excluded.cost_usd,
                   files_changed_json = excluded.files_changed_json,
                   agent_session_id = excluded.agent_session_id,
                   continuation_count = excluded.continuation_count,
                   reported = excluded.reported,
                   updated_at = excluded.updated_at",
        )
        .bind(job.id)
        .bind(&job.profile_name)
        .bind(&job.command)
        .bind(&job.user_intent)
        .bind(job.status)
        .bind(&job.working_directory)
        .bind(job.conversation_id)
        .bind(job.pid)
        .bind(job.exit_code)
        .bind(&job.result_summary)
        .bind(&job.raw_output_path)
        .bind(&job.error_message)
        .bind(job.started_at)
        .bind(job.completed_at)
        .bind(job.cost_usd)
        .bind(&job.files_changed_json)
        .bind(&job.agent_session_id)
        .bind(job.continuation_count)
        .bind(job.reported)
        .bind(job.created_at)
        .bind(job.updated_at)
        .execute(&self.pool)
        .await
        .map_err(AppError::Database)?;

        debug!(job_id = %job.id, status = %job.status, profile = %job.profile_name, "agent_job_repo.saved");
        Ok(job.clone())
    }

    async fn get_by_id(&self, id: Uuid) -> Result<Option<AgentJob>, AppError> {
        sqlx::query_as::<_, AgentJob>("SELECT * FROM agent_jobs WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn find_by_status(&self, status: AgentJobStatus) -> Result<Vec<AgentJob>, AppError> {
        sqlx::query_as::<_, AgentJob>(
            "SELECT * FROM agent_jobs WHERE status = ?1 ORDER BY created_at DESC",
        )
        .bind(status.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn find_unreported_terminal(&self) -> Result<Vec<AgentJob>, AppError> {
        sqlx::query_as::<_, AgentJob>(
            "SELECT * FROM agent_jobs WHERE status IN ('completed', 'failed', 'cancelled') AND reported = 0 ORDER BY completed_at ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn mark_reported(&self, id: Uuid) -> Result<(), AppError> {
        sqlx::query("UPDATE agent_jobs SET reported = 1 WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(AppError::Database)?;
        info!(job_id = %id, "agent_job_repo.mark_reported");
        Ok(())
    }

    async fn get_running_count(&self) -> Result<i64, AppError> {
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM agent_jobs WHERE status = 'running'")
                .fetch_one(&self.pool)
                .await
                .map_err(AppError::Database)?;
        Ok(count.0)
    }
}
