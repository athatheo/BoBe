use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::app_state::AppState;
use crate::error::AppError;

#[derive(Debug, Serialize)]
pub struct CaptureStatusResponse {
    pub capturing: bool,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct CaptureOnceResponse {
    pub success: bool,
    pub active_window: Option<String>,
    pub message: String,
}

/// POST /api/capture/start
pub async fn start_capture(
    State(state): State<Arc<AppState>>,
) -> Result<Json<CaptureStatusResponse>, AppError> {
    tracing::info!("api.capture_start_requested");
    state.runtime_session.start_capture().await;

    Ok(Json(CaptureStatusResponse {
        capturing: true,
        message: "Capture loop started".into(),
    }))
}

/// POST /api/capture/stop
pub async fn stop_capture(
    State(state): State<Arc<AppState>>,
) -> Result<Json<CaptureStatusResponse>, AppError> {
    tracing::info!("api.capture_stop_requested");
    state.runtime_session.stop_capture().await;

    Ok(Json(CaptureStatusResponse {
        capturing: false,
        message: "Capture loop stopped".into(),
    }))
}

/// POST /api/capture/once
pub async fn capture_once(
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
