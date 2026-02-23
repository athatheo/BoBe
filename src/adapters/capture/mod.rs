pub mod capture_result;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use chrono::Utc;
use tracing::{debug, info};

use crate::error::AppError;
use capture_result::CaptureResult;

const CAPTURE_DIR: &str = "/tmp";

/// Screen capture service for taking screenshots on macOS.
///
/// Uses the `screencapture` CLI tool to capture the screen, then reads
/// the PNG file and returns it as raw bytes. Active window title is
/// retrieved via AppleScript.
pub struct ScreenCapture;

impl ScreenCapture {
    pub fn new() -> Self {
        Self
    }

    /// Capture the primary display screenshot.
    ///
    /// Shells out to `screencapture -x <path>` (macOS) then reads the
    /// resulting PNG file.
    pub async fn capture_screen(&self) -> Result<CaptureResult, AppError> {
        let timestamp = Utc::now();
        debug!("capture.started");

        let image = tokio::task::spawn_blocking(take_screenshot)
            .await
            .map_err(|e| AppError::Capture(format!("Screenshot task panicked: {e}")))?
            .map_err(|e| AppError::Capture(format!("Screenshot failed: {e}")))?;

        let active_window = tokio::task::spawn_blocking(get_active_window)
            .await
            .map_err(|e| AppError::Capture(format!("Window title task panicked: {e}")))?;

        info!(
            image_size_kb = image.len() / 1024,
            active_window = ?active_window,
            "capture.complete"
        );

        Ok(CaptureResult {
            image,
            active_window,
            timestamp,
            source: "screen".into(),
        })
    }

    /// Encode a CaptureResult's image as base64 PNG.
    pub fn encode_base64(result: &CaptureResult) -> String {
        BASE64.encode(&result.image)
    }
}

impl Default for ScreenCapture {
    fn default() -> Self {
        Self::new()
    }
}

/// Take a screenshot using macOS `screencapture` (blocking).
fn take_screenshot() -> Result<Vec<u8>, std::io::Error> {
    let capture_path = format!("{}/bobe_capture_{}.png", CAPTURE_DIR, uuid::Uuid::new_v4());
    let output = std::process::Command::new("screencapture")
        .args(["-x", &capture_path])
        .output()?;

    if !output.status.success() {
        let _ = std::fs::remove_file(&capture_path);
        return Err(std::io::Error::other(format!(
            "screencapture exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    let data = std::fs::read(&capture_path)?;
    if let Err(e) = std::fs::remove_file(&capture_path) {
        tracing::warn!(path = %capture_path, error = %e, "capture.temp_file_cleanup_failed");
    }
    Ok(data)
}

/// Get the active window title via AppleScript (blocking, macOS only).
fn get_active_window() -> Option<String> {
    let applescript =
        r#"tell application "System Events" to get name of first process whose frontmost is true"#;

    let output = std::process::Command::new("osascript")
        .args(["-e", applescript])
        .output()
        .ok()?;

    if output.status.success() {
        let title = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if title.is_empty() { None } else { Some(title) }
    } else {
        None
    }
}
