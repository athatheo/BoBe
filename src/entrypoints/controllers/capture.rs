use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::app_state::AppState;
use crate::error::AppError;

// ── Response ────────────────────────────────────────────────────────────────

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

// ── Handlers ────────────────────────────────────────────────────────────────

/// POST /api/capture/start
pub async fn start_capture(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<CaptureStatusResponse>, AppError> {
    // Full RuntimeSession integration comes later
    tracing::info!("api.capture_start_requested");

    Ok(Json(CaptureStatusResponse {
        capturing: true,
        message: "Capture loop started".into(),
    }))
}

/// POST /api/capture/stop
pub async fn stop_capture(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<CaptureStatusResponse>, AppError> {
    tracing::info!("api.capture_stop_requested");

    Ok(Json(CaptureStatusResponse {
        capturing: false,
        message: "Capture loop stopped".into(),
    }))
}

/// POST /api/capture/once
pub async fn capture_once(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<CaptureOnceResponse>, AppError> {
    // Full ScreenCapture integration comes later
    tracing::info!("api.capture_once_requested");

    Ok(Json(CaptureOnceResponse {
        success: false,
        active_window: None,
        message: "Screen capture not yet wired".into(),
    }))
}
