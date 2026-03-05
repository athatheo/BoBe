use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::db::SoulRepository;
use crate::error::AppError;
use crate::models::soul::Soul;

#[derive(Debug, Serialize)]
pub(crate) struct SoulResponse {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) content: String,
    pub(crate) enabled: bool,
    pub(crate) is_default: bool,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SoulListResponse {
    pub(crate) souls: Vec<SoulResponse>,
    pub(crate) count: usize,
    pub(crate) enabled_count: usize,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SoulCreateRequest {
    pub(crate) name: String,
    pub(crate) content: String,
    #[serde(default = "super::default_true")]
    pub(crate) enabled: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SoulUpdateRequest {
    pub(crate) content: Option<String>,
    pub(crate) enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SoulActionResponse {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) enabled: bool,
    pub(crate) message: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SoulListQuery {
    #[serde(default)]
    pub(crate) enabled_only: bool,
}

fn soul_to_response(soul: &Soul) -> SoulResponse {
    SoulResponse {
        id: soul.id.to_string(),
        name: soul.name.clone(),
        content: soul.content.clone(),
        enabled: soul.enabled,
        is_default: soul.is_default,
        created_at: soul.created_at,
        updated_at: soul.updated_at,
    }
}

async fn set_soul_enabled(
    soul_repo: &Arc<dyn SoulRepository>,
    soul_id: Uuid,
    enabled: bool,
) -> Result<Json<SoulActionResponse>, AppError> {
    let soul = soul_repo
        .update(soul_id, None, Some(enabled), None, None)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Soul {soul_id} not found")))?;

    if enabled {
        tracing::info!(soul_id = %soul_id, "soul.enabled");
    } else {
        tracing::info!(soul_id = %soul_id, "soul.disabled");
    }

    Ok(Json(SoulActionResponse {
        id: soul_id.to_string(),
        name: soul.name,
        enabled,
        message: if enabled {
            "Soul enabled".into()
        } else {
            "Soul disabled".into()
        },
    }))
}

pub(crate) async fn list_souls(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SoulListQuery>,
) -> Result<Json<SoulListResponse>, AppError> {
    let souls = if params.enabled_only {
        state.soul_repo.find_enabled().await?
    } else {
        state.soul_repo.get_all().await?
    };

    let enabled_count = souls.iter().filter(|s| s.enabled).count();

    Ok(Json(SoulListResponse {
        count: souls.len(),
        enabled_count,
        souls: souls.iter().map(soul_to_response).collect(),
    }))
}

pub(crate) async fn get_soul(
    State(state): State<Arc<AppState>>,
    Path(soul_id): Path<Uuid>,
) -> Result<Json<SoulResponse>, AppError> {
    let soul = state
        .soul_repo
        .get_by_id(soul_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Soul {soul_id} not found")))?;

    Ok(Json(soul_to_response(&soul)))
}

pub(crate) async fn create_soul(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SoulCreateRequest>,
) -> Result<(StatusCode, Json<SoulResponse>), AppError> {
    if body.name.is_empty() {
        return Err(AppError::Validation("name must not be empty".into()));
    }
    if body.content.len() < 10 {
        return Err(AppError::Validation(
            "content must be at least 10 characters".into(),
        ));
    }

    if state.soul_repo.get_by_name(&body.name).await?.is_some() {
        return Err(AppError::Validation(format!(
            "Soul with name '{}' already exists",
            body.name
        )));
    }

    let mut soul = Soul::new(body.name, body.content, false);
    soul.enabled = body.enabled;
    let saved = state.soul_repo.save(&soul).await?;

    tracing::info!(soul_id = %saved.id, name = %saved.name, "soul.created");

    Ok((StatusCode::CREATED, Json(soul_to_response(&saved))))
}

/// Copy-on-write: editing a default soul preserves the original as a disabled copy.
pub(crate) async fn update_soul(
    State(state): State<Arc<AppState>>,
    Path(soul_id): Path<Uuid>,
    Json(body): Json<SoulUpdateRequest>,
) -> Result<Json<SoulResponse>, AppError> {
    let soul = state
        .soul_repo
        .get_by_id(soul_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Soul {soul_id} not found")))?;

    let is_content_edit_of_default = body.content.is_some() && soul.is_default;

    let updated = if is_content_edit_of_default {
        let original_name = soul.name.clone();
        let original_content = soul.content.clone();
        let edited_name = format!("{original_name} (edited)");

        let updated = state
            .soul_repo
            .update(
                soul_id,
                body.content.as_deref(),
                body.enabled,
                Some(false),
                Some(&edited_name),
            )
            .await?;

        let default_copy = Soul {
            id: Uuid::new_v4(),
            name: original_name,
            content: original_content,
            enabled: false,
            is_default: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        state.soul_repo.save(&default_copy).await?;

        tracing::info!(
            soul_id = %soul_id,
            edited_name = %edited_name,
            "soul.copy_on_write",
        );

        updated
    } else {
        state
            .soul_repo
            .update(soul_id, body.content.as_deref(), body.enabled, None, None)
            .await?
    };

    let updated = updated.ok_or_else(|| AppError::NotFound(format!("Soul {soul_id} not found")))?;

    tracing::info!(soul_id = %soul_id, "soul.updated");
    Ok(Json(soul_to_response(&updated)))
}

pub(crate) async fn get_soul_by_name(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<SoulResponse>, AppError> {
    let soul = state
        .soul_repo
        .get_by_name(&name)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Soul '{name}' not found")))?;

    Ok(Json(soul_to_response(&soul)))
}

pub(crate) async fn enable_soul(
    State(state): State<Arc<AppState>>,
    Path(soul_id): Path<Uuid>,
) -> Result<Json<SoulActionResponse>, AppError> {
    set_soul_enabled(&state.soul_repo, soul_id, true).await
}

pub(crate) async fn disable_soul(
    State(state): State<Arc<AppState>>,
    Path(soul_id): Path<Uuid>,
) -> Result<Json<SoulActionResponse>, AppError> {
    set_soul_enabled(&state.soul_repo, soul_id, false).await
}

pub(crate) async fn delete_soul(
    State(state): State<Arc<AppState>>,
    Path(soul_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let soul = state
        .soul_repo
        .get_by_id(soul_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Soul {soul_id} not found")))?;

    if soul.is_default {
        return Err(AppError::Validation(
            "Cannot delete default soul. Disable it instead.".into(),
        ));
    }

    if !state.soul_repo.delete(soul_id).await? {
        return Err(AppError::NotFound(format!("Soul {soul_id} not found")));
    }

    tracing::info!(soul_id = %soul_id, name = %soul.name, "soul.deleted");

    Ok(StatusCode::NO_CONTENT)
}
