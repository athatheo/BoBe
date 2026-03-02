use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::error::AppError;
use crate::models::goal_plan::{GoalPlan, GoalPlanStep};
use crate::models::types::{GoalPlanStatus, GoalStatus};
use crate::services::goal_worker::manager::GoalWorkerStatus;

// ── Schemas ─────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct GoalIdRequest {
    pub goal_id: String,
}

#[derive(Debug, Serialize)]
pub struct GoalPlanResponse {
    pub id: String,
    pub goal_id: String,
    pub summary: String,
    pub status: String,
    pub failure_count: i32,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub steps: Option<Vec<GoalPlanStepResponse>>,
}

#[derive(Debug, Serialize)]
pub struct GoalPlanStepResponse {
    pub id: String,
    pub plan_id: String,
    pub step_order: i32,
    pub content: String,
    pub status: String,
    pub result: Option<String>,
    pub error: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct GoalPlanListResponse {
    pub plans: Vec<GoalPlanResponse>,
    pub count: usize,
}

#[derive(Debug, Deserialize)]
pub struct GoalPlanListQuery {
    pub goal_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PlanActionResponse {
    pub id: String,
    pub status: String,
    pub message: String,
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn plan_to_response(plan: &GoalPlan, steps: Option<Vec<GoalPlanStep>>) -> GoalPlanResponse {
    GoalPlanResponse {
        id: plan.id.to_string(),
        goal_id: plan.goal_id.to_string(),
        summary: plan.summary.clone(),
        status: plan.status.as_str().to_owned(),
        failure_count: plan.failure_count,
        last_error: plan.last_error.clone(),
        created_at: plan.created_at,
        updated_at: plan.updated_at,
        steps: steps.map(|s| s.iter().map(step_to_response).collect()),
    }
}

fn step_to_response(step: &GoalPlanStep) -> GoalPlanStepResponse {
    GoalPlanStepResponse {
        id: step.id.to_string(),
        plan_id: step.plan_id.to_string(),
        step_order: step.step_order,
        content: step.content.clone(),
        status: step.status.as_str().to_owned(),
        result: step.result.clone(),
        error: step.error.clone(),
        started_at: step.started_at,
        completed_at: step.completed_at,
        created_at: step.created_at,
    }
}

// ── Handlers ────────────────────────────────────────────────────────────────

/// GET /api/goal-plans
pub async fn list_goal_plans(
    State(state): State<Arc<AppState>>,
    Query(params): Query<GoalPlanListQuery>,
) -> Result<Json<GoalPlanListResponse>, AppError> {
    let plans = if let Some(ref goal_id_str) = params.goal_id {
        let goal_id = goal_id_str
            .parse::<uuid::Uuid>()
            .map_err(|_| AppError::Validation(format!("Invalid goal_id: {goal_id_str}")))?;
        state.goal_plan_repo.get_plans_for_goal(goal_id).await?
    } else {
        state.goal_plan_repo.get_pending_approval_plans().await?
    };

    let count = plans.len();
    let responses: Vec<GoalPlanResponse> =
        plans.iter().map(|p| plan_to_response(p, None)).collect();

    Ok(Json(GoalPlanListResponse {
        plans: responses,
        count,
    }))
}

/// GET /api/goal-plans/:plan_id
pub async fn get_goal_plan(
    State(state): State<Arc<AppState>>,
    Path(plan_id): Path<String>,
) -> Result<Json<GoalPlanResponse>, AppError> {
    let plan_uuid = plan_id
        .parse::<uuid::Uuid>()
        .map_err(|_| AppError::Validation(format!("Invalid plan_id: {plan_id}")))?;

    let plan = state
        .goal_plan_repo
        .get_plan(plan_uuid)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Plan {plan_id} not found")))?;

    let steps = state.goal_plan_repo.get_steps_for_plan(plan_uuid).await?;

    Ok(Json(plan_to_response(&plan, Some(steps))))
}

/// POST /api/goal-plans/:plan_id/approve
pub async fn approve_goal_plan(
    State(state): State<Arc<AppState>>,
    Path(plan_id): Path<String>,
) -> Result<Json<PlanActionResponse>, AppError> {
    let plan_uuid = plan_id
        .parse::<uuid::Uuid>()
        .map_err(|_| AppError::Validation(format!("Invalid plan_id: {plan_id}")))?;

    let plan = state
        .goal_plan_repo
        .get_plan(plan_uuid)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Plan {plan_id} not found")))?;

    if plan.status != GoalPlanStatus::PendingApproval {
        return Err(AppError::Validation(format!(
            "Plan is not pending approval (current status: {})",
            plan.status.as_str()
        )));
    }

    state
        .goal_plan_repo
        .update_plan_status(plan_uuid, GoalPlanStatus::Approved, None)
        .await?;

    Ok(Json(PlanActionResponse {
        id: plan_id,
        status: "approved".to_string(),
        message: "Plan approved and queued for execution".to_string(),
    }))
}

/// POST /api/goal-plans/:plan_id/reject
pub async fn reject_goal_plan(
    State(state): State<Arc<AppState>>,
    Path(plan_id): Path<String>,
) -> Result<Json<PlanActionResponse>, AppError> {
    let plan_uuid = plan_id
        .parse::<uuid::Uuid>()
        .map_err(|_| AppError::Validation(format!("Invalid plan_id: {plan_id}")))?;

    let plan = state
        .goal_plan_repo
        .get_plan(plan_uuid)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Plan {plan_id} not found")))?;

    if plan.status != GoalPlanStatus::PendingApproval {
        return Err(AppError::Validation(format!(
            "Plan is not pending approval (current status: {})",
            plan.status.as_str()
        )));
    }

    state
        .goal_plan_repo
        .update_plan_status(plan_uuid, GoalPlanStatus::Rejected, None)
        .await?;

    // Return goal to Active so it can be re-planned
    state
        .goal_repo
        .update_status(plan.goal_id, Some(GoalStatus::Active), None)
        .await?;

    Ok(Json(PlanActionResponse {
        id: plan_id,
        status: "rejected".to_string(),
        message: "Plan rejected; goal returned to active".to_string(),
    }))
}

/// POST /api/goal-plans/pause
pub async fn pause_goal(
    State(state): State<Arc<AppState>>,
    Json(body): Json<GoalIdRequest>,
) -> Result<Json<PlanActionResponse>, AppError> {
    let goal_id = body
        .goal_id
        .parse::<uuid::Uuid>()
        .map_err(|_| AppError::Validation(format!("Invalid goal_id: {}", body.goal_id)))?;

    let goal = state
        .goal_repo
        .get_by_id(goal_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Goal {goal_id} not found")))?;

    if goal.status != GoalStatus::Active {
        return Err(AppError::Validation(format!(
            "Goal is not active (current status: {})",
            goal.status
        )));
    }

    state
        .goal_repo
        .update_status(goal_id, Some(GoalStatus::Paused), None)
        .await?;

    Ok(Json(PlanActionResponse {
        id: goal_id.to_string(),
        status: "paused".to_string(),
        message: "Goal paused".to_string(),
    }))
}

/// POST /api/goal-plans/resume
pub async fn resume_goal(
    State(state): State<Arc<AppState>>,
    Json(body): Json<GoalIdRequest>,
) -> Result<Json<PlanActionResponse>, AppError> {
    let goal_id = body
        .goal_id
        .parse::<uuid::Uuid>()
        .map_err(|_| AppError::Validation(format!("Invalid goal_id: {}", body.goal_id)))?;

    let goal = state
        .goal_repo
        .get_by_id(goal_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Goal {goal_id} not found")))?;

    if goal.status != GoalStatus::Paused {
        return Err(AppError::Validation(format!(
            "Goal is not paused (current status: {})",
            goal.status
        )));
    }

    state
        .goal_repo
        .update_status(goal_id, Some(GoalStatus::Active), None)
        .await?;

    Ok(Json(PlanActionResponse {
        id: goal_id.to_string(),
        status: "active".to_string(),
        message: "Goal resumed".to_string(),
    }))
}

/// GET /api/goal-plans/status
pub async fn goal_worker_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<GoalWorkerStatus>, AppError> {
    let cfg = state.config();
    let enabled = cfg.goal_worker.enabled;
    let max_concurrent = cfg.goal_worker.max_concurrent;

    let active_goals = state.goal_repo.find_active(true).await?;
    let pending_plans = state.goal_plan_repo.get_pending_approval_plans().await?;

    Ok(Json(GoalWorkerStatus {
        enabled,
        max_concurrent,
        active_goals_count: active_goals.len(),
        pending_approval_count: pending_plans.len(),
    }))
}
