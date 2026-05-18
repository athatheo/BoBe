use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use serde::Serialize;

use crate::app_state::AppState;
use crate::error::AppError;

#[derive(Debug, Serialize)]
pub(crate) struct AuthStatusResponse {
    pub(crate) is_authenticated: bool,
    pub(crate) auth_type: Option<String>,
    pub(crate) host: Option<String>,
    pub(crate) login: Option<String>,
    pub(crate) status_message: Option<String>,
    /// Bundled CLI path; Swift uses this to open Terminal for sign-in.
    pub(crate) cli_path: Option<String>,
    pub(crate) cli_version: Option<String>,
}

pub(crate) async fn get_auth_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<AuthStatusResponse>, AppError> {
    // `Client::start` extracts the bundled CLI as a side effect; required
    // before `embeddedcli::path()` returns Some.
    let client = state
        .runtime
        .workers
        .client_handle()
        .ensure_started()
        .await
        .map_err(|e| AppError::Internal(format!("auth_status: client start failed: {e}")))?;

    let status = client
        .get_auth_status()
        .await
        .map_err(|e| AppError::Internal(format!("auth_status: get_auth_status failed: {e}")))?;

    let cli_path =
        github_copilot_sdk::embeddedcli::path().map(|p| p.to_string_lossy().into_owned());
    let cli_version =
        github_copilot_sdk::embeddedcli::bundled_version().map(std::string::ToString::to_string);

    Ok(Json(AuthStatusResponse {
        is_authenticated: status.is_authenticated,
        auth_type: status.auth_type,
        host: status.host,
        login: status.login,
        status_message: status.status_message,
        cli_path,
        cli_version,
    }))
}
