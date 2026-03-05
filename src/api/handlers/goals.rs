use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::constants::{GOAL_CONTENT_MAX_LENGTH, GOAL_CONTENT_MIN_LENGTH};
use crate::error::AppError;
use crate::models::goal::Goal;
use crate::models::types::{GoalPriority, GoalSource, GoalStatus};

#[derive(Debug, Serialize)]
pub(crate) struct GoalResponse {
    pub(crate) id: String,
    pub(crate) content: String,
    pub(crate) status: String,
    pub(crate) priority: String,
    pub(crate) source: String,
    pub(crate) enabled: bool,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub(crate) struct GoalListResponse {
    pub(crate) goals: Vec<GoalResponse>,
    pub(crate) count: usize,
    pub(crate) active_count: usize,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GoalCreateRequest {
    pub(crate) content: String,
    #[serde(default = "default_priority")]
    pub(crate) priority: String,
    #[serde(default = "super::default_true")]
    pub(crate) enabled: bool,
}

fn default_priority() -> String {
    "medium".into()
}

#[derive(Debug, Deserialize)]
pub(crate) struct GoalUpdateRequest {
    pub(crate) content: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) priority: Option<String>,
    pub(crate) enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
pub(crate) struct GoalActionResponse {
    pub(crate) id: String,
    pub(crate) status: String,
    pub(crate) message: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GoalListQuery {
    pub(crate) status: Option<String>,
    #[serde(default)]
    pub(crate) include_archived: bool,
}

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
        "paused" => Ok(GoalStatus::Paused),
        "completed" => Ok(GoalStatus::Completed),
        "archived" => Ok(GoalStatus::Archived),
        _ => Err(AppError::Validation(format!(
            "Invalid status '{s}'. Valid: active, paused, completed, archived"
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

pub(crate) async fn list_goals(
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

pub(crate) async fn get_goal(
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

pub(crate) async fn create_goal(
    State(state): State<Arc<AppState>>,
    Json(body): Json<GoalCreateRequest>,
) -> Result<(StatusCode, Json<GoalResponse>), AppError> {
    if body.content.len() < GOAL_CONTENT_MIN_LENGTH || body.content.len() > GOAL_CONTENT_MAX_LENGTH
    {
        return Err(AppError::Validation(format!(
            "content must be between {GOAL_CONTENT_MIN_LENGTH} and {GOAL_CONTENT_MAX_LENGTH} characters"
        )));
    }

    let priority = parse_goal_priority(&body.priority)?;
    let mut goal = Goal::new(body.content, GoalSource::User, priority);
    goal.enabled = body.enabled;

    let saved = state.goal_repo.save(&goal).await?;

    tracing::info!(goal_id = %saved.id, "goal.created");

    Ok((StatusCode::CREATED, Json(goal_to_response(&saved))))
}

pub(crate) async fn update_goal(
    State(state): State<Arc<AppState>>,
    Path(goal_id): Path<Uuid>,
    Json(body): Json<GoalUpdateRequest>,
) -> Result<Json<GoalResponse>, AppError> {
    let existing_goal = state
        .goal_repo
        .get_by_id(goal_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Goal {goal_id} not found")))?;

    let mut updated_goal = Some(existing_goal);
    if let Some(ref content) = body.content {
        updated_goal = state.goals_service.update_content(goal_id, content).await?;
        if updated_goal.is_none() {
            return Err(AppError::NotFound(format!("Goal {goal_id} not found")));
        }
    }

    let status = body.status.as_deref().map(parse_goal_status).transpose()?;
    let priority = body
        .priority
        .as_deref()
        .map(parse_goal_priority)
        .transpose()?;

    if status.is_some() || priority.is_some() || body.enabled.is_some() {
        updated_goal = state
            .goal_repo
            .update_fields(
                goal_id,
                None, // content already handled above
                status,
                priority,
                None,
                body.enabled,
            )
            .await?;
    }

    let goal =
        updated_goal.ok_or_else(|| AppError::NotFound(format!("Goal {goal_id} not found")))?;

    tracing::info!(goal_id = %goal_id, "goal.updated");
    Ok(Json(goal_to_response(&goal)))
}

pub(crate) async fn complete_goal(
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

pub(crate) async fn archive_goal(
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

pub(crate) async fn delete_goal(
    State(state): State<Arc<AppState>>,
    Path(goal_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    if !state.goal_repo.delete(goal_id).await? {
        return Err(AppError::NotFound(format!("Goal {goal_id} not found")));
    }

    tracing::info!(goal_id = %goal_id, "goal.deleted");

    Ok(StatusCode::NO_CONTENT)
}
