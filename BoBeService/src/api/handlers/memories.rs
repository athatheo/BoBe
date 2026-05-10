use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::error::AppError;

#[derive(Debug, Serialize)]
pub(crate) struct MemoryResponse {
    pub(crate) content: String,
    pub(crate) bytes: usize,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MemoryUpdateRequest {
    pub(crate) content: String,
}

pub(crate) async fn get_memory(
    State(state): State<Arc<AppState>>,
) -> Result<Json<MemoryResponse>, AppError> {
    let content = state.memory_file.read().await?;
    let bytes = content.len();
    Ok(Json(MemoryResponse { content, bytes }))
}

pub(crate) async fn update_memory(
    State(state): State<Arc<AppState>>,
    Json(body): Json<MemoryUpdateRequest>,
) -> Result<Json<MemoryResponse>, AppError> {
    state.memory_file.replace_all(body.content.clone()).await?;
    let bytes = body.content.len();
    Ok(Json(MemoryResponse {
        content: body.content,
        bytes,
    }))
}
