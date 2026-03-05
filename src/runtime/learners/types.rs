use std::collections::HashMap;

use chrono::{DateTime, Utc};

use crate::models::ids::ObservationId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LearnerObservationSource {
    Capture,
    Message,
}

impl LearnerObservationSource {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Capture => "capture",
            Self::Message => "message",
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct LearnerObservation {
    pub(crate) source: LearnerObservationSource,
    pub(crate) timestamp: DateTime<Utc>,
    pub(crate) screenshot: Option<Vec<u8>>,
    pub(crate) text: Option<String>,
    pub(crate) active_window: Option<String>,
    pub(crate) metadata: HashMap<String, serde_json::Value>,
}

impl LearnerObservation {
    pub(crate) fn capture(screenshot: Vec<u8>, active_window: Option<String>) -> Self {
        Self {
            source: LearnerObservationSource::Capture,
            timestamp: Utc::now(),
            screenshot: Some(screenshot),
            text: None,
            active_window,
            metadata: HashMap::new(),
        }
    }

    pub(crate) fn message(text: String) -> Self {
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

#[derive(Debug, thiserror::Error)]
pub(crate) enum LearnerError {
    #[error("Wrong observation source: expected {expected}, got {got}")]
    WrongSource { expected: String, got: String },

    #[error("Missing required data: {0}")]
    MissingData(String),

    #[error("Embedding error: {0}")]
    Embedding(String),

    #[error("LLM error: {0}")]
    Llm(String),

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

#[derive(Debug)]
pub(crate) enum LearnerResult {
    Stored { observation_id: ObservationId },
}
