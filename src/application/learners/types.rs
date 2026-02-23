//! Base types for learners.

use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Sources that produce observations for learning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LearnerObservationSource {
    Capture,
    Message,
}

impl LearnerObservationSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Capture => "capture",
            Self::Message => "message",
        }
    }
}

/// Raw observation from a source, before learner processing.
#[derive(Debug, Clone)]
pub struct LearnerObservation {
    pub source: LearnerObservationSource,
    pub timestamp: DateTime<Utc>,
    pub screenshot: Option<Vec<u8>>,
    pub text: Option<String>,
    pub active_window: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl LearnerObservation {
    pub fn capture(screenshot: Vec<u8>, active_window: Option<String>) -> Self {
        Self {
            source: LearnerObservationSource::Capture,
            timestamp: Utc::now(),
            screenshot: Some(screenshot),
            text: None,
            active_window,
            metadata: HashMap::new(),
        }
    }

    pub fn message(text: String) -> Self {
        Self {
            source: LearnerObservationSource::Message,
            timestamp: Utc::now(),
            screenshot: None,
            text: Some(text),
            active_window: None,
            metadata: HashMap::new(),
        }
    }
}

/// Error type for learner operations.
#[derive(Debug, thiserror::Error)]
pub enum LearnerError {
    #[error("Wrong observation source: expected {expected}, got {got}")]
    WrongSource { expected: String, got: String },

    #[error("Missing required data: {0}")]
    MissingData(String),

    #[error("Embedding error: {0}")]
    Embedding(String),

    #[error("LLM error: {0}")]
    Llm(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Storage error: {0}")]
    Storage(String),
}

impl From<crate::error::AppError> for LearnerError {
    fn from(e: crate::error::AppError) -> Self {
        match e {
            crate::error::AppError::Embedding(msg) => LearnerError::Embedding(msg),
            crate::error::AppError::Llm(msg) => LearnerError::Llm(msg),
            crate::error::AppError::LlmTimeout(msg) => LearnerError::Llm(format!("timeout: {msg}")),
            other => LearnerError::Storage(other.to_string()),
        }
    }
}

/// Result of a single learner operation.
#[derive(Debug)]
pub enum LearnerResult {
    /// Observation was stored successfully.
    Stored { observation_id: uuid::Uuid },
    /// Learning was skipped (e.g., no useful content).
    Skipped { reason: String },
}
