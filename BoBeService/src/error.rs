use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Capture error: {0}")]
    Capture(String),

    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    /// Distinct variant so install-status doesn't string-match against Conflict.
    #[error("Canceled: {0}")]
    Canceled(String),

    #[error("MCP error: {0}")]
    Mcp(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl axum::response::IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        use axum::http::StatusCode;

        let (status, code) = match &self {
            AppError::Validation(_) => (StatusCode::BAD_REQUEST, "VALIDATION_ERROR"),
            AppError::Conflict(_) => (StatusCode::CONFLICT, "CONFLICT"),
            AppError::NotFound(_) => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            AppError::Database(_) => (StatusCode::SERVICE_UNAVAILABLE, "DATABASE_ERROR"),
            AppError::ServiceUnavailable(_) => {
                (StatusCode::SERVICE_UNAVAILABLE, "SERVICE_UNAVAILABLE")
            }
            AppError::Canceled(_) => (StatusCode::CONFLICT, "CANCELED"),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR"),
        };

        let body = serde_json::json!({
            "error": {
                "code": code,
                "message": self.to_string(),
            }
        });

        (status, axum::Json(body)).into_response()
    }
}
