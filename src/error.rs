use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("LLM unavailable: {0}")]
    LlmUnavailable(String),

    #[error("LLM timeout: {0}")]
    LlmTimeout(String),

    #[error("LLM rate limited: {0}")]
    LlmRateLimited(String),

    #[error("LLM error: {0}")]
    Llm(String),

    #[error("Circuit open: {0}")]
    CircuitOpen(String),

    #[error("Tool error: {0}")]
    Tool(String),

    #[error("Tool call loop error: {0}")]
    ToolCallLoop(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Capture error: {0}")]
    Capture(String),

    #[error("Embedding error: {0}")]
    Embedding(String),

    #[error("MCP error: {0}")]
    Mcp(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("HTTP client error: {0}")]
    HttpClient(#[from] reqwest::Error),

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
            AppError::NotFound(_) => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            AppError::LlmUnavailable(_) | AppError::CircuitOpen(_) => {
                (StatusCode::SERVICE_UNAVAILABLE, "LLM_UNAVAILABLE")
            }
            AppError::LlmTimeout(_) => (StatusCode::GATEWAY_TIMEOUT, "LLM_TIMEOUT"),
            AppError::LlmRateLimited(_) => (StatusCode::TOO_MANY_REQUESTS, "LLM_RATE_LIMITED"),
            AppError::Database(_) => (StatusCode::SERVICE_UNAVAILABLE, "DATABASE_ERROR"),
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
