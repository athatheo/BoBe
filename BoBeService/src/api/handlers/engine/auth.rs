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
    /// Resolved CLI path used by the in-app sign-in flow.
    pub(crate) cli_path: Option<String>,
}

pub(crate) async fn get_auth_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<AuthStatusResponse>, AppError> {
    // `Client::start` resolves the installed CLI and starts the JSON-RPC server.
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

    let cli_path = crate::copilot::client::installed_cli_path()
        .map(|path| path.to_string_lossy().into_owned());

    Ok(Json(AuthStatusResponse {
        is_authenticated: status.is_authenticated,
        auth_type: status.auth_type,
        host: status.host,
        login: status.login,
        status_message: status.status_message,
        cli_path,
    }))
}
