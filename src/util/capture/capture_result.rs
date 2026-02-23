use chrono::{DateTime, Utc};

/// Result from a screen capture operation.
#[derive(Debug, Clone)]
pub struct CaptureResult {
    /// PNG image data (raw bytes).
    pub image: Vec<u8>,
    /// Active window title (if available).
    pub active_window: Option<String>,
    /// When the capture was taken.
    pub timestamp: DateTime<Utc>,
    /// Source of the capture (e.g. "screen", "region").
    pub source: String,
}
