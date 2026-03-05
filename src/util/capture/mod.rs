pub(crate) mod capture_result;

use tracing::{debug, info};

use crate::error::AppError;
use capture_result::CaptureResult;

const CAPTURE_DIR: &str = "/tmp";

pub(crate) struct ScreenCapture;

impl ScreenCapture {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) async fn capture_screen(&self) -> Result<CaptureResult, AppError> {
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
        })
    }
}

impl Default for ScreenCapture {
    fn default() -> Self {
        Self::new()
    }
}

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

    // Detect blank frame — macOS returns all-black PNG when screen recording
    // permission is not granted. Check a sample of pixels.
    if data.len() > 1000 && is_blank_image(&data) {
        return Err(std::io::Error::other(
            "Screen capture returned a blank frame — screen recording permission may not be granted",
        ));
    }

    Ok(data)
}

fn is_blank_image(png_data: &[u8]) -> bool {
    let start = png_data.len().min(100);
    let end = png_data.len().saturating_sub(12);
    if end <= start {
        return false;
    }
    let sample: Vec<u8> = png_data[start..end]
        .iter()
        .step_by(4)
        .take(500)
        .copied()
        .collect();
    if sample.len() < 100 {
        return false;
    }
    sample.iter().all(|&b| b == 0)
}

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
