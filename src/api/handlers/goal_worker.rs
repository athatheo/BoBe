use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::error::AppError;
use crate::models::goal_plan::{GoalPlan, GoalPlanStep};
use crate::models::types::{GoalPlanStatus, GoalStatus};

// ── Schemas ─────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ActionResponseRequest {
    pub request_id: String,
    pub response: String,
}

#[derive(Debug, Serialize)]
pub struct ActionResponseResult {
    pub delivered: bool,
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

/// POST /api/goal-worker/action-response
pub async fn submit_action_response(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ActionResponseRequest>,
) -> Result<Json<ActionResponseResult>, AppError> {
    let delivered = state
        .ask_user_bridge
        .submit_response(&body.request_id, body.response)
        .await?;

    Ok(Json(ActionResponseResult { delivered }))
}

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
    let responses: Vec<GoalPlanResponse> = plans
        .iter()
        .map(|p| plan_to_response(p, None))
        .collect();

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
