use std::sync::Arc;

use axum::Json;
use axum::extract::State;

use crate::app_state::AppState;
use crate::error::AppError;
use crate::services::mcp_config_service::{self as mcp_svc, McpConfigMutationRequest};

pub(crate) async fn get_mcp_config(
    State(state): State<Arc<AppState>>,
) -> Result<Json<mcp_svc::McpConfigDocumentResponse>, AppError> {
    Ok(Json(mcp_svc::get_document(state.as_ref()).await?))
}

/// Pure schema check, no subprocesses spawned.
pub(crate) async fn validate_mcp_config(
    State(state): State<Arc<AppState>>,
    Json(body): Json<McpConfigMutationRequest>,
) -> Result<Json<mcp_svc::McpConfigValidateResponse>, AppError> {
    Ok(Json(mcp_svc::validate_document(state.as_ref(), &body)?))
}

pub(crate) async fn save_mcp_config(
    State(state): State<Arc<AppState>>,
    Json(body): Json<McpConfigMutationRequest>,
) -> Result<Json<mcp_svc::McpConfigSaveResponse>, AppError> {
    Ok(Json(mcp_svc::save_document(state.as_ref(), body).await?))
}

pub(crate) async fn reset_mcp_config(
    State(state): State<Arc<AppState>>,
) -> Result<Json<mcp_svc::McpConfigResetResponse>, AppError> {
    Ok(Json(mcp_svc::reset_document(state.as_ref()).await?))
}
