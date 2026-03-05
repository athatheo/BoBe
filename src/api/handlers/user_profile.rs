use std::sync::Arc;

use crate::app_state::AppState;
use crate::db::UserProfileRepository;
use crate::error::AppError;
use crate::models::ids::UserProfileId;
use crate::models::user_profile::UserProfile;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub(crate) struct UserProfileResponse {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) content: String,
    pub(crate) enabled: bool,
    pub(crate) is_default: bool,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub(crate) struct UserProfileListResponse {
    pub(crate) profiles: Vec<UserProfileResponse>,
    pub(crate) count: usize,
    pub(crate) enabled_count: usize,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UserProfileCreateRequest {
    pub(crate) name: String,
    pub(crate) content: String,
    #[serde(default = "super::default_true")]
    pub(crate) enabled: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UserProfileUpdateRequest {
    pub(crate) content: Option<String>,
    pub(crate) enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
pub(crate) struct UserProfileActionResponse {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) enabled: bool,
    pub(crate) message: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UserProfileListQuery {
    #[serde(default)]
    pub(crate) enabled_only: bool,
}

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

async fn set_profile_enabled(
    user_profile_repo: &Arc<dyn UserProfileRepository>,
    profile_id: UserProfileId,
    enabled: bool,
) -> Result<Json<UserProfileActionResponse>, AppError> {
    let profile = user_profile_repo
        .update(profile_id, None, Some(enabled))
        .await?
        .ok_or_else(|| AppError::NotFound(format!("User profile {profile_id} not found")))?;

    if enabled {
        tracing::info!(profile_id = %profile_id, "user_profile.enabled");
    } else {
        tracing::info!(profile_id = %profile_id, "user_profile.disabled");
    }

    Ok(Json(UserProfileActionResponse {
        id: profile_id.to_string(),
        name: profile.name,
        enabled,
        message: if enabled {
            "User profile enabled".into()
        } else {
            "User profile disabled".into()
        },
    }))
}

pub(crate) async fn list_profiles(
    State(state): State<Arc<AppState>>,
    Query(params): Query<UserProfileListQuery>,
) -> Result<Json<UserProfileListResponse>, AppError> {
    let profiles = if params.enabled_only {
        state.user_profile_repo.find_enabled().await?
    } else {
        state.user_profile_repo.get_all().await?
    };

    let enabled_count = profiles.iter().filter(|p| p.enabled).count();

    Ok(Json(UserProfileListResponse {
        count: profiles.len(),
        enabled_count,
        profiles: profiles.iter().map(profile_to_response).collect(),
    }))
}

pub(crate) async fn create_profile(
    State(state): State<Arc<AppState>>,
    Json(body): Json<UserProfileCreateRequest>,
) -> Result<(StatusCode, Json<UserProfileResponse>), AppError> {
    if body.name.is_empty() {
        return Err(AppError::Validation("name must not be empty".into()));
    }
    if body.content.len() < 10 {
        return Err(AppError::Validation(
            "content must be at least 10 characters".into(),
        ));
    }

    if state
        .user_profile_repo
        .get_by_name(&body.name)
        .await?
        .is_some()
    {
        return Err(AppError::Validation(format!(
            "User profile with name '{}' already exists",
            body.name
        )));
    }

    let mut profile = UserProfile::new(body.name, body.content, false);
    profile.enabled = body.enabled;
    let saved = state.user_profile_repo.save(&profile).await?;

    tracing::info!(profile_id = %saved.id, name = %saved.name, "user_profile.created");

    Ok((StatusCode::CREATED, Json(profile_to_response(&saved))))
}

pub(crate) async fn get_profile(
    State(state): State<Arc<AppState>>,
    Path(profile_id): Path<UserProfileId>,
) -> Result<Json<UserProfileResponse>, AppError> {
    let profile = state
        .user_profile_repo
        .get_by_id(profile_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("User profile {profile_id} not found")))?;

    Ok(Json(profile_to_response(&profile)))
}

pub(crate) async fn get_profile_by_name(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<UserProfileResponse>, AppError> {
    let profile = state
        .user_profile_repo
        .get_by_name(&name)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("User profile '{name}' not found")))?;

    Ok(Json(profile_to_response(&profile)))
}

pub(crate) async fn update_profile(
    State(state): State<Arc<AppState>>,
    Path(profile_id): Path<UserProfileId>,
    Json(body): Json<UserProfileUpdateRequest>,
) -> Result<Json<UserProfileResponse>, AppError> {
    state
        .user_profile_repo
        .get_by_id(profile_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("User profile {profile_id} not found")))?;

    let updated = state
        .user_profile_repo
        .update(profile_id, body.content.as_deref(), body.enabled)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("User profile {profile_id} not found")))?;

    tracing::info!(profile_id = %profile_id, "user_profile.updated");
    Ok(Json(profile_to_response(&updated)))
}

pub(crate) async fn enable_profile(
    State(state): State<Arc<AppState>>,
    Path(profile_id): Path<UserProfileId>,
) -> Result<Json<UserProfileActionResponse>, AppError> {
    set_profile_enabled(&state.user_profile_repo, profile_id, true).await
}

pub(crate) async fn disable_profile(
    State(state): State<Arc<AppState>>,
    Path(profile_id): Path<UserProfileId>,
) -> Result<Json<UserProfileActionResponse>, AppError> {
    set_profile_enabled(&state.user_profile_repo, profile_id, false).await
}

pub(crate) async fn delete_profile(
    State(state): State<Arc<AppState>>,
    Path(profile_id): Path<UserProfileId>,
) -> Result<StatusCode, AppError> {
    let profile = state
        .user_profile_repo
        .get_by_id(profile_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("User profile {profile_id} not found")))?;

    if profile.is_default {
        return Err(AppError::Validation(
            "Cannot delete default user profile. Disable it instead.".into(),
        ));
    }

    let deleted = state.user_profile_repo.delete(profile_id).await?;
    if !deleted {
        return Err(AppError::NotFound(format!(
            "User profile {profile_id} not found"
        )));
    }

    tracing::info!(profile_id = %profile_id, name = %profile.name, "user_profile.deleted");

    Ok(StatusCode::NO_CONTENT)
}
