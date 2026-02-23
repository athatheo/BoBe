use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;


use crate::app_state::AppState;
use crate::domain::user_profile::UserProfile;
use crate::error::AppError;


// ── Schemas ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct UserProfileResponse {
    pub id: String,
    pub name: String,
    pub content: String,
    pub enabled: bool,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct UserProfileListResponse {
    pub profiles: Vec<UserProfileResponse>,
    pub count: usize,
    pub enabled_count: usize,
}

#[derive(Debug, Deserialize)]
pub struct UserProfileCreateRequest {
    pub name: String,
    pub content: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct UserProfileUpdateRequest {
    pub content: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct UserProfileActionResponse {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct UserProfileListQuery {
    #[serde(default)]
    pub enabled_only: bool,
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn profile_to_response(profile: &UserProfile) -> UserProfileResponse {
    UserProfileResponse {
        id: profile.id.to_string(),
        name: profile.name.clone(),
        content: profile.content.clone(),
        enabled: profile.enabled,
        is_default: profile.is_default,
        created_at: profile.created_at,
        updated_at: profile.updated_at,
    }
}

// ── Handlers ────────────────────────────────────────────────────────────────

/// GET /api/user-profiles
pub async fn list_profiles(
    State(state): State<Arc<AppState>>,
    Query(params): Query<UserProfileListQuery>,
) -> Result<Json<UserProfileListResponse>, AppError> {
    let repo = state.user_profile_repo.clone();

    let profiles = if params.enabled_only {
        repo.find_enabled().await?
    } else {
        repo.get_all().await?
    };

    let enabled_count = profiles.iter().filter(|p| p.enabled).count();

    Ok(Json(UserProfileListResponse {
        count: profiles.len(),
        enabled_count,
        profiles: profiles.iter().map(profile_to_response).collect(),
    }))
}

/// POST /api/user-profiles
pub async fn create_profile(
    State(state): State<Arc<AppState>>,
    Json(body): Json<UserProfileCreateRequest>,
) -> Result<Json<UserProfileResponse>, AppError> {
    if body.name.is_empty() {
        return Err(AppError::Validation("name must not be empty".into()));
    }
    if body.content.len() < 10 {
        return Err(AppError::Validation(
            "content must be at least 10 characters".into(),
        ));
    }

    let repo = state.user_profile_repo.clone();

    if repo.get_by_name(&body.name).await?.is_some() {
        return Err(AppError::Validation(format!(
            "User profile with name '{}' already exists",
            body.name
        )));
    }

    let profile = UserProfile::new(body.name, body.content, false);
    let saved = repo.save(&profile).await?;

    tracing::info!(profile_id = %saved.id, name = %saved.name, "user_profile.created");

    Ok(Json(profile_to_response(&saved)))
}

/// PUT /api/user-profiles/:id
pub async fn update_profile(
    State(state): State<Arc<AppState>>,
    Path(profile_id): Path<Uuid>,
    Json(body): Json<UserProfileUpdateRequest>,
) -> Result<Json<UserProfileResponse>, AppError> {
    let repo = state.user_profile_repo.clone();

    repo.get_by_id(profile_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("User profile {profile_id} not found")))?;

    let updated = repo
        .update(profile_id, body.content.as_deref(), body.enabled)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("User profile {profile_id} not found")))?;

    tracing::info!(profile_id = %profile_id, "user_profile.updated");
    Ok(Json(profile_to_response(&updated)))
}
