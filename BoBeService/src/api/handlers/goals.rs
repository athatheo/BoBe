//! HTTP handlers for `/goals/*`.
//!
//! Goals are file-backed: each lives at `~/.bobe/goals/<id>.md`. The
//! API exposes the `GoalDoc` shape (rich sections + metadata header)
//! to the SwiftUI overlay so users can see + edit Why It Matters,
//! Notes, etc. directly. The chat agent edits the same files via the
//! SDK's Read/Write/Edit tools — both paths hit `GoalsService`.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::error::AppError;
use crate::models::ids::GoalId;
use crate::models::types::GoalStatus;
use crate::services::goals::goal_md::GoalDoc;

#[derive(Debug, Serialize)]
pub(crate) struct GoalResponse {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) status: String,
    pub(crate) priority: u8,
    pub(crate) summary: String,
    pub(crate) why_it_matters: String,
    pub(crate) how_working_on_it: String,
    pub(crate) patterns_observed: String,
    pub(crate) attitude_feelings: String,
    pub(crate) open_questions: String,
    pub(crate) notes: String,
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
    pub(crate) title: String,
    #[serde(default)]
    pub(crate) summary: String,
    #[serde(default)]
    pub(crate) why_it_matters: String,
    #[serde(default = "default_priority")]
    pub(crate) priority: u8,
}

fn default_priority() -> u8 {
    2
}

#[derive(Debug, Deserialize)]
pub(crate) struct GoalUpdateRequest {
    pub(crate) title: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) priority: Option<u8>,
    pub(crate) summary: Option<String>,
    pub(crate) why_it_matters: Option<String>,
    pub(crate) notes: Option<String>,
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
    /// Defaults to false; pass `?include_archived=true` to include
    /// `archived` goals in `list_goals`.
    #[serde(default)]
    pub(crate) include_archived: bool,
}

fn doc_to_response(doc: &GoalDoc) -> GoalResponse {
    GoalResponse {
        id: doc.id.to_string(),
        title: doc.title.clone(),
        status: doc.status.as_str().to_owned(),
        priority: doc.priority,
        summary: doc.summary.clone(),
        why_it_matters: doc.why_it_matters.clone(),
        how_working_on_it: doc.how_working_on_it.clone(),
        patterns_observed: doc.patterns_observed.clone(),
        attitude_feelings: doc.attitude_feelings.clone(),
        open_questions: doc.open_questions.clone(),
        notes: doc.notes.clone(),
        created_at: doc.created_at,
        updated_at: doc.updated_at,
    }
}

fn parse_goal_status(s: &str) -> Result<GoalStatus, AppError> {
    match s.to_lowercase().as_str() {
        "active" => Ok(GoalStatus::Active),
        "paused" => Ok(GoalStatus::Paused),
        "completed" => Ok(GoalStatus::Completed),
        "archived" => Ok(GoalStatus::Archived),
        _ => Err(AppError::Validation(format!(
            "Invalid status '{s}'. Valid: active, paused, completed, archived"
        ))),
    }
}

pub(crate) async fn list_goals(
    State(state): State<Arc<AppState>>,
    Query(params): Query<GoalListQuery>,
) -> Result<Json<GoalListResponse>, AppError> {
    let mut goals = if let Some(ref status_str) = params.status {
        let status = parse_goal_status(status_str)?;
        let all = state.goals_service.list_all().await?;
        all.into_iter().filter(|g| g.status == status).collect()
    } else {
        state.goals_service.list_all().await?
    };

    if !params.include_archived && params.status.is_none() {
        goals.retain(|g| g.status != GoalStatus::Archived);
    }

    let active_count = goals
        .iter()
        .filter(|g| g.status == GoalStatus::Active)
        .count();
    Ok(Json(GoalListResponse {
        count: goals.len(),
        active_count,
        goals: goals.iter().map(doc_to_response).collect(),
    }))
}

pub(crate) async fn get_goal(
    State(state): State<Arc<AppState>>,
    Path(goal_id): Path<GoalId>,
) -> Result<Json<GoalResponse>, AppError> {
    let doc = state
        .goals_service
        .get(goal_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Goal {goal_id} not found")))?;
    Ok(Json(doc_to_response(&doc)))
}

pub(crate) async fn create_goal(
    State(state): State<Arc<AppState>>,
    Json(body): Json<GoalCreateRequest>,
) -> Result<(StatusCode, Json<GoalResponse>), AppError> {
    if body.title.trim().is_empty() {
        return Err(AppError::Validation("title must not be empty".into()));
    }
    if body.priority > 5 {
        return Err(AppError::Validation(
            "priority must be 0-5 (5 = highest)".into(),
        ));
    }

    let mut doc = GoalDoc::new(body.title.trim(), body.summary);
    doc.priority = body.priority;
    doc.why_it_matters = body.why_it_matters;

    let saved = state.goals_service.create(doc).await?;
    Ok((StatusCode::CREATED, Json(doc_to_response(&saved))))
}

pub(crate) async fn update_goal(
    State(state): State<Arc<AppState>>,
    Path(goal_id): Path<GoalId>,
    Json(body): Json<GoalUpdateRequest>,
) -> Result<Json<GoalResponse>, AppError> {
    let status = body.status.as_deref().map(parse_goal_status).transpose()?;
    if let Some(p) = body.priority
        && p > 5
    {
        return Err(AppError::Validation(
            "priority must be 0-5 (5 = highest)".into(),
        ));
    }

    let updated = state
        .goals_service
        .update(
            goal_id,
            body.title,
            status,
            body.priority,
            body.summary,
            body.why_it_matters,
            body.notes,
        )
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Goal {goal_id} not found")))?;

    Ok(Json(doc_to_response(&updated)))
}

pub(crate) async fn complete_goal(
    State(state): State<Arc<AppState>>,
    Path(goal_id): Path<GoalId>,
) -> Result<Json<GoalActionResponse>, AppError> {
    let updated = state
        .goals_service
        .set_status(goal_id, GoalStatus::Completed)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Goal {goal_id} not found")))?;
    Ok(Json(GoalActionResponse {
        id: goal_id.to_string(),
        status: updated.status.as_str().to_owned(),
        message: "Goal marked as completed".into(),
    }))
}

pub(crate) async fn archive_goal(
    State(state): State<Arc<AppState>>,
    Path(goal_id): Path<GoalId>,
) -> Result<Json<GoalActionResponse>, AppError> {
    let updated = state
        .goals_service
        .set_status(goal_id, GoalStatus::Archived)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Goal {goal_id} not found")))?;
    Ok(Json(GoalActionResponse {
        id: goal_id.to_string(),
        status: updated.status.as_str().to_owned(),
        message: "Goal archived".into(),
    }))
}

pub(crate) async fn delete_goal(
    State(state): State<Arc<AppState>>,
    Path(goal_id): Path<GoalId>,
) -> Result<StatusCode, AppError> {
    if !state.goals_service.delete(goal_id).await? {
        return Err(AppError::NotFound(format!("Goal {goal_id} not found")));
    }
    Ok(StatusCode::NO_CONTENT)
}
