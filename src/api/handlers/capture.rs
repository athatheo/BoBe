use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use serde::Serialize;

use crate::app_state::AppState;
use crate::error::AppError;

#[derive(Debug, Serialize)]
pub(crate) struct CaptureStatusResponse {
    pub(crate) capturing: bool,
    pub(crate) message: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct CaptureOnceResponse {
    pub(crate) success: bool,
    pub(crate) active_window: Option<String>,
    pub(crate) message: String,
}

pub(crate) async fn start_capture(
    State(state): State<Arc<AppState>>,
) -> Result<Json<CaptureStatusResponse>, AppError> {
    tracing::info!("api.capture_start_requested");
    state.runtime_session.start_capture().await;

    Ok(Json(CaptureStatusResponse {
        capturing: true,
        message: "Capture loop started".into(),
    }))
}

pub(crate) async fn stop_capture(
    State(state): State<Arc<AppState>>,
) -> Result<Json<CaptureStatusResponse>, AppError> {
    tracing::info!("api.capture_stop_requested");
    state.runtime_session.stop_capture().await;

    Ok(Json(CaptureStatusResponse {
        capturing: false,
        message: "Capture loop stopped".into(),
    }))
}

pub(crate) async fn capture_once(
    State(state): State<Arc<AppState>>,
) -> Result<Json<CaptureOnceResponse>, AppError> {
    tracing::info!("api.capture_once_requested");

    match state.screen_capture.capture_screen().await {
        Ok(result) => Ok(Json(CaptureOnceResponse {
            success: true,
            active_window: result.active_window,
            message: "Capture completed".into(),
        })),
        Err(e) => Ok(Json(CaptureOnceResponse {
            success: false,
            active_window: None,
            message: format!("Capture failed: {e}"),
        })),
    }
}
