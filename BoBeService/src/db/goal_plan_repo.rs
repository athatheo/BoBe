use crate::db::GoalPlanRepository;
use crate::error::AppError;
use crate::models::goal_plan::{GoalPlan, GoalPlanStep};
use crate::models::ids::{GoalId, GoalPlanId, GoalPlanStepId};
use crate::models::types::{GoalPlanStatus, GoalPlanStepStatus};
use async_trait::async_trait;
use chrono::Utc;
use sqlx::SqlitePool;
use tracing::{debug, info, warn};

pub(crate) struct SqliteGoalPlanRepo {
    pool: SqlitePool,
}

impl SqliteGoalPlanRepo {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl GoalPlanRepository for SqliteGoalPlanRepo {
    async fn create_plan(
        &self,
        goal_id: GoalId,
        summary: &str,
        status: GoalPlanStatus,
    ) -> Result<GoalPlan, AppError> {
        let id = GoalPlanId::new();
        let now = Utc::now();

        sqlx::query(
            "INSERT INTO goal_plans (id, goal_id, summary, status, failure_count, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6)",
        )
        .bind(id)
        .bind(goal_id)
        .bind(summary)
        .bind(status.as_str())
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(AppError::Database)?;

        debug!(plan_id = %id, goal_id = %goal_id, status = %status, "goal_plan_repo.created");

        Ok(GoalPlan {
            id,
            goal_id,
            summary: summary.to_string(),
            status,
            failure_count: 0,
            last_error: None,
            created_at: now,
            updated_at: now,
        })
    }

    async fn get_plan(&self, plan_id: GoalPlanId) -> Result<Option<GoalPlan>, AppError> {
        sqlx::query_as::<_, GoalPlan>("SELECT * FROM goal_plans WHERE id = ?1")
            .bind(plan_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn get_plans_for_goal(&self, goal_id: GoalId) -> Result<Vec<GoalPlan>, AppError> {
        sqlx::query_as::<_, GoalPlan>(
            "SELECT * FROM goal_plans WHERE goal_id = ?1 ORDER BY created_at DESC",
        )
        .bind(goal_id)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn get_active_plan_for_goal(
        &self,
        goal_id: GoalId,
    ) -> Result<Option<GoalPlan>, AppError> {
        sqlx::query_as::<_, GoalPlan>(
            "SELECT * FROM goal_plans WHERE goal_id = ?1 AND status IN ('approved', 'auto_approved', 'in_progress') ORDER BY created_at DESC LIMIT 1",
        )
        .bind(goal_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn update_plan_status(
        &self,
        plan_id: GoalPlanId,
        status: GoalPlanStatus,
        error: Option<&str>,
    ) -> Result<Option<GoalPlan>, AppError> {
        let existing = self.get_plan(plan_id).await?;
        if existing.is_none() {
            warn!(plan_id = %plan_id, "goal_plan_repo.update_status.not_found");
            return Ok(None);
        }

        let now = Utc::now();
        if let Some(err) = error {
            sqlx::query(
                "UPDATE goal_plans SET status = ?1, last_error = ?2, failure_count = failure_count + 1, updated_at = ?3 WHERE id = ?4",
            )
            .bind(status.as_str())
            .bind(err)
            .bind(now)
            .bind(plan_id)
            .execute(&self.pool)
            .await
            .map_err(AppError::Database)?;
        } else {
            sqlx::query("UPDATE goal_plans SET status = ?1, updated_at = ?2 WHERE id = ?3")
                .bind(status.as_str())
                .bind(now)
                .bind(plan_id)
                .execute(&self.pool)
                .await
                .map_err(AppError::Database)?;
        }

        info!(plan_id = %plan_id, status = %status, has_error = error.is_some(), "goal_plan_repo.status_updated");
        self.get_plan(plan_id).await
    }

    async fn get_pending_approval_plans(&self) -> Result<Vec<GoalPlan>, AppError> {
        sqlx::query_as::<_, GoalPlan>(
            "SELECT * FROM goal_plans WHERE status = 'pending_approval' ORDER BY created_at ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn get_expired_pending_plans(
        &self,
        timeout_minutes: i64,
    ) -> Result<Vec<GoalPlan>, AppError> {
        sqlx::query_as::<_, GoalPlan>(
            "SELECT * FROM goal_plans WHERE status = 'pending_approval' AND created_at < datetime('now', ?1) ORDER BY created_at ASC",
        )
        .bind(format!("-{timeout_minutes} minutes"))
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn create_step(
        &self,
        plan_id: GoalPlanId,
        step_order: i32,
        content: &str,
    ) -> Result<GoalPlanStep, AppError> {
        let id = GoalPlanStepId::new();
        let now = Utc::now();

        sqlx::query(
            "INSERT INTO goal_plan_steps (id, plan_id, step_order, content, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(id)
        .bind(plan_id)
        .bind(step_order)
        .bind(content)
        .bind(GoalPlanStepStatus::Pending.as_str())
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(AppError::Database)?;

        debug!(step_id = %id, plan_id = %plan_id, step_order, "goal_plan_repo.step_created");

        Ok(GoalPlanStep {
            id,
            plan_id,
            step_order,
            content: content.to_string(),
            status: GoalPlanStepStatus::Pending,
            result: None,
            error: None,
            started_at: None,
            completed_at: None,
            created_at: now,
        })
    }

    async fn update_step_status(
        &self,
        step_id: GoalPlanStepId,
        status: GoalPlanStepStatus,
        result: Option<&str>,
        error: Option<&str>,
    ) -> Result<Option<GoalPlanStep>, AppError> {
        let existing =
            sqlx::query_as::<_, GoalPlanStep>("SELECT * FROM goal_plan_steps WHERE id = ?1")
                .bind(step_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(AppError::Database)?;

        if existing.is_none() {
            warn!(step_id = %step_id, "goal_plan_repo.update_step.not_found");
            return Ok(None);
        }

        let now = Utc::now();
        let started_at = if status == GoalPlanStepStatus::InProgress {
            Some(now)
        } else {
            None
        };
        let completed_at = if status.is_terminal() {
            Some(now)
        } else {
            None
        };

        sqlx::query(
            "UPDATE goal_plan_steps SET status = ?1, result = COALESCE(?2, result), error = COALESCE(?3, error), started_at = COALESCE(?4, started_at), completed_at = COALESCE(?5, completed_at) WHERE id = ?6",
        )
        .bind(status.as_str())
        .bind(result)
        .bind(error)
        .bind(started_at)
        .bind(completed_at)
        .bind(step_id)
        .execute(&self.pool)
        .await
        .map_err(AppError::Database)?;

        info!(step_id = %step_id, status = %status, "goal_plan_repo.step_updated");

        sqlx::query_as::<_, GoalPlanStep>("SELECT * FROM goal_plan_steps WHERE id = ?1")
            .bind(step_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn get_steps_for_plan(&self, plan_id: GoalPlanId) -> Result<Vec<GoalPlanStep>, AppError> {
        sqlx::query_as::<_, GoalPlanStep>(
            "SELECT * FROM goal_plan_steps WHERE plan_id = ?1 ORDER BY step_order ASC",
        )
        .bind(plan_id)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::Database)
    }
}
