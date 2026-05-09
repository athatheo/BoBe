//! HTTP handlers for `/memory`.
//!
//! Memory is a single living document at `~/.bobe/memory.md` (the
//! durable narrative store; pruned nightly by the Consolidate
//! worker). The pre-pivot row-oriented endpoints (list, search by
//! embedding, enable/disable per-row, etc.) are gone — there are no
//! rows. The UI now reads + writes the file as a whole.

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::error::AppError;

#[derive(Debug, Serialize)]
pub(crate) struct MemoryResponse {
    /// Full contents of `memory.md`.
    pub(crate) content: String,
    /// Size in bytes — useful for the UI to gauge how close we are
    /// to the consolidation worker's prune threshold.
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
