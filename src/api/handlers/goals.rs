use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::models::goal::Goal;
use crate::models::types::{GoalPriority, GoalSource, GoalStatus};
use crate::error::AppError;

// ── Schemas ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct GoalResponse {
    pub id: String,
    pub content: String,
    pub status: String,
    pub priority: String,
    pub source: String,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct GoalListResponse {
    pub goals: Vec<GoalResponse>,
    pub count: usize,
    pub active_count: usize,
}

#[derive(Debug, Deserialize)]
pub struct GoalCreateRequest {
    pub content: String,
    #[serde(default = "default_priority")]
    pub priority: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_priority() -> String {
    "medium".into()
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct GoalUpdateRequest {
    pub content: Option<String>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct GoalActionResponse {
    pub id: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct GoalListQuery {
    pub status: Option<String>,
    #[serde(default)]
    pub include_archived: bool,
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn goal_to_response(goal: &Goal) -> GoalResponse {
    GoalResponse {
        id: goal.id.to_string(),
        content: goal.content.clone(),
        status: goal.status.as_str().to_owned(),
        priority: goal.priority.as_str().to_owned(),
        source: goal.source.as_str().to_owned(),
        enabled: goal.enabled,
        created_at: goal.created_at,
        updated_at: goal.updated_at,
    }
}

fn parse_goal_status(s: &str) -> Result<GoalStatus, AppError> {
    match s {
        "active" => Ok(GoalStatus::Active),
        "completed" => Ok(GoalStatus::Completed),
        "archived" => Ok(GoalStatus::Archived),
        _ => Err(AppError::Validation(format!(
            "Invalid status '{s}'. Valid: active, completed, archived"
        ))),
    }
}

fn parse_goal_priority(s: &str) -> Result<GoalPriority, AppError> {
    match s {
        "high" => Ok(GoalPriority::High),
        "medium" => Ok(GoalPriority::Medium),
        "low" => Ok(GoalPriority::Low),
        _ => Err(AppError::Validation(format!(
            "Invalid priority '{s}'. Valid: high, medium, low"
        ))),
    }
}

// ── Handlers ────────────────────────────────────────────────────────────────

/// GET /api/goals
pub async fn list_goals(
    State(state): State<Arc<AppState>>,
    Query(params): Query<GoalListQuery>,
) -> Result<Json<GoalListResponse>, AppError> {
    let goals = if let Some(ref status_str) = params.status {
        let status = parse_goal_status(status_str)?;
        state.goal_repo.find_by_status(status, false).await?
    } else {
        state.goal_repo.get_all(params.include_archived).await?
    };

    let active_count = goals.iter().filter(|g| g.is_active()).count();

    Ok(Json(GoalListResponse {
        count: goals.len(),
        active_count,
        goals: goals.iter().map(goal_to_response).collect(),
    }))
}

/// GET /api/goals/:id
pub async fn get_goal(
    State(state): State<Arc<AppState>>,
    Path(goal_id): Path<Uuid>,
) -> Result<Json<GoalResponse>, AppError> {
    let goal = state
        .goal_repo
        .get_by_id(goal_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Goal {goal_id} not found")))?;

    Ok(Json(goal_to_response(&goal)))
}

/// POST /api/goals
pub async fn create_goal(
    State(state): State<Arc<AppState>>,
    Json(body): Json<GoalCreateRequest>,
) -> Result<(StatusCode, Json<GoalResponse>), AppError> {
    if body.content.len() < 3 {
        return Err(AppError::Validation(
            "content must be at least 3 characters".into(),
        ));
    }

    let priority = parse_goal_priority(&body.priority)?;
    let goal = Goal::new(body.content, GoalSource::User, priority);

    let saved = state.goal_repo.save(&goal).await?;

    tracing::info!(goal_id = %saved.id, "goal.created");

    Ok((StatusCode::CREATED, Json(goal_to_response(&saved))))
}

/// PUT /api/goals/:id
pub async fn update_goal(
    State(state): State<Arc<AppState>>,
    Path(goal_id): Path<Uuid>,
    Json(body): Json<GoalUpdateRequest>,
) -> Result<Json<GoalResponse>, AppError> {
    state
        .goal_repo
        .get_by_id(goal_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Goal {goal_id} not found")))?;

    let status = body.status.as_deref().map(parse_goal_status).transpose()?;
    let priority = body
        .priority
        .as_deref()
        .map(parse_goal_priority)
        .transpose()?;

    let updated = state
        .goal_repo
        .update_fields(
            goal_id,
            body.content.as_deref(),
            status,
            priority,
            None,
            body.enabled,
        )
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Goal {goal_id} not found")))?;

    tracing::info!(goal_id = %goal_id, "goal.updated");
    Ok(Json(goal_to_response(&updated)))
}

/// POST /api/goals/:id/complete
pub async fn complete_goal(
    State(state): State<Arc<AppState>>,
    Path(goal_id): Path<Uuid>,
) -> Result<Json<GoalActionResponse>, AppError> {
    state
        .goal_repo
        .get_by_id(goal_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Goal {goal_id} not found")))?;

    let updated = state
        .goal_repo
        .update_status(goal_id, Some(GoalStatus::Completed), None)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Goal {goal_id} could not be updated")))?;

    tracing::info!(goal_id = %goal_id, "goal.completed");

    Ok(Json(GoalActionResponse {
        id: goal_id.to_string(),
        status: updated.status.as_str().to_owned(),
        message: "Goal marked as completed".into(),
    }))
}

/// POST /api/goals/:id/archive
pub async fn archive_goal(
    State(state): State<Arc<AppState>>,
    Path(goal_id): Path<Uuid>,
) -> Result<Json<GoalActionResponse>, AppError> {
    state
        .goal_repo
        .get_by_id(goal_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Goal {goal_id} not found")))?;

    let updated = state
        .goal_repo
        .update_status(goal_id, Some(GoalStatus::Archived), None)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Goal {goal_id} could not be updated")))?;

    tracing::info!(goal_id = %goal_id, "goal.archived");

    Ok(Json(GoalActionResponse {
        id: goal_id.to_string(),
        status: updated.status.as_str().to_owned(),
        message: "Goal archived".into(),
    }))
}

/// DELETE /api/goals/:id
pub async fn delete_goal(
    State(state): State<Arc<AppState>>,
    Path(goal_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    if !state.goal_repo.delete(goal_id).await? {
        return Err(AppError::NotFound(format!("Goal {goal_id} not found")));
    }

    tracing::info!(goal_id = %goal_id, "goal.deleted");

    Ok(StatusCode::NO_CONTENT)
}
