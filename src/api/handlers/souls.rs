use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::error::AppError;
use crate::models::soul::Soul;

// ── Schemas ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct SoulResponse {
    pub id: String,
    pub name: String,
    pub content: String,
    pub enabled: bool,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct SoulListResponse {
    pub souls: Vec<SoulResponse>,
    pub count: usize,
    pub enabled_count: usize,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct SoulCreateRequest {
    pub name: String,
    pub content: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct SoulUpdateRequest {
    pub content: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct SoulActionResponse {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct SoulListQuery {
    #[serde(default)]
    pub enabled_only: bool,
}

// ── Helpers ─────────────────────────────────────────────────────────────────

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

// ── Handlers ────────────────────────────────────────────────────────────────

/// GET /api/souls
pub async fn list_souls(
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

/// GET /api/souls/:id
pub async fn get_soul(
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

/// POST /api/souls
pub async fn create_soul(
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

    // Check for duplicate name
    if state.soul_repo.get_by_name(&body.name).await?.is_some() {
        return Err(AppError::Validation(format!(
            "Soul with name '{}' already exists",
            body.name
        )));
    }

    let soul = Soul::new(body.name, body.content, false);
    let saved = state.soul_repo.save(&soul).await?;

    tracing::info!(soul_id = %saved.id, name = %saved.name, "soul.created");

    Ok((StatusCode::CREATED, Json(soul_to_response(&saved))))
}

/// PATCH /api/souls/:id
///
/// Copy-on-write for default souls: editing a default soul renames it
/// to "<name> (edited)" and preserves the original as a disabled copy.
pub async fn update_soul(
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

        // Rename + apply edits in one update
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

        // Preserve original as disabled copy
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

/// GET /api/souls/by-name/:name
pub async fn get_soul_by_name(
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

/// POST /api/souls/:id/enable
pub async fn enable_soul(
    State(state): State<Arc<AppState>>,
    Path(soul_id): Path<Uuid>,
) -> Result<Json<SoulActionResponse>, AppError> {
    let soul = state
        .soul_repo
        .get_by_id(soul_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Soul {soul_id} not found")))?;

    state
        .soul_repo
        .update(soul_id, None, Some(true), None, None)
        .await?;
    tracing::info!(soul_id = %soul_id, "soul.enabled");

    Ok(Json(SoulActionResponse {
        id: soul_id.to_string(),
        name: soul.name,
        enabled: true,
        message: "Soul enabled".into(),
    }))
}

/// POST /api/souls/:id/disable
pub async fn disable_soul(
    State(state): State<Arc<AppState>>,
    Path(soul_id): Path<Uuid>,
) -> Result<Json<SoulActionResponse>, AppError> {
    let soul = state
        .soul_repo
        .get_by_id(soul_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Soul {soul_id} not found")))?;

    state
        .soul_repo
        .update(soul_id, None, Some(false), None, None)
        .await?;
    tracing::info!(soul_id = %soul_id, "soul.disabled");

    Ok(Json(SoulActionResponse {
        id: soul_id.to_string(),
        name: soul.name,
        enabled: false,
        message: "Soul disabled".into(),
    }))
}

/// DELETE /api/souls/:id
pub async fn delete_soul(
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
