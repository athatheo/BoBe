use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;


use crate::app_state::AppState;
use crate::domain::memory::Memory;
use crate::domain::types::{MemorySource, MemoryType};
use crate::error::AppError;


// ── Schemas ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct MemoryResponse {
    pub id: String,
    pub content: String,
    pub memory_type: String,
    pub category: String,
    pub source: String,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct MemoryListResponse {
    pub memories: Vec<MemoryResponse>,
    pub count: usize,
    pub total: i64,
}

#[derive(Debug, Deserialize)]
pub struct MemoryCreateRequest {
    pub content: String,
    #[serde(default = "default_category")]
    pub category: String,
    #[serde(default = "default_memory_type")]
    pub memory_type: String,
}

fn default_category() -> String {
    "general".into()
}

fn default_memory_type() -> String {
    "explicit".into()
}

#[derive(Debug, Deserialize)]
pub struct MemoryUpdateRequest {
    pub content: Option<String>,
    pub category: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct MemoryActionResponse {
    pub id: String,
    pub enabled: bool,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct MemoryListQuery {
    pub memory_type: Option<String>,
    pub category: Option<String>,
    pub source: Option<String>,
    #[serde(default)]
    pub enabled_only: bool,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    100
}

#[derive(Debug, Deserialize)]
pub struct MemorySearchRequest {
    pub query: String,
    #[serde(default = "default_search_limit")]
    pub limit: i64,
}

fn default_search_limit() -> i64 {
    20
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn memory_to_response(memory: &Memory) -> MemoryResponse {
    MemoryResponse {
        id: memory.id.to_string(),
        content: memory.content.clone(),
        memory_type: memory.memory_type.clone(),
        category: memory.category.clone(),
        source: memory.source.clone(),
        enabled: memory.enabled,
        created_at: memory.created_at,
        updated_at: memory.updated_at,
    }
}

fn parse_memory_type(s: &str) -> Result<MemoryType, AppError> {
    match s {
        "short_term" => Ok(MemoryType::ShortTerm),
        "long_term" => Ok(MemoryType::LongTerm),
        "explicit" => Ok(MemoryType::Explicit),
        _ => Err(AppError::Validation(format!(
            "Invalid memory_type '{s}'. Valid: short_term, long_term, explicit"
        ))),
    }
}

// ── Handlers ────────────────────────────────────────────────────────────────

/// GET /api/memories
pub async fn list_memories(
    State(state): State<Arc<AppState>>,
    Query(params): Query<MemoryListQuery>,
) -> Result<Json<MemoryListResponse>, AppError> {
    let repo = state.memory_repo.clone();

    let (memories, total) = repo
        .find_all(
            params.memory_type.as_deref(),
            params.category.as_deref(),
            params.source.as_deref(),
            params.enabled_only,
            params.limit,
            params.offset,
        )
        .await?;

    Ok(Json(MemoryListResponse {
        count: memories.len(),
        total,
        memories: memories.iter().map(memory_to_response).collect(),
    }))
}

/// GET /api/memories/:id
pub async fn get_memory(
    State(state): State<Arc<AppState>>,
    Path(memory_id): Path<Uuid>,
) -> Result<Json<MemoryResponse>, AppError> {
    let repo = state.memory_repo.clone();
    let memory = repo
        .get_by_id(memory_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Memory {memory_id} not found")))?;

    Ok(Json(memory_to_response(&memory)))
}

/// POST /api/memories
pub async fn create_memory(
    State(state): State<Arc<AppState>>,
    Json(body): Json<MemoryCreateRequest>,
) -> Result<Json<MemoryResponse>, AppError> {
    if body.content.len() < 3 {
        return Err(AppError::Validation(
            "content must be at least 3 characters".into(),
        ));
    }

    let memory_type = parse_memory_type(&body.memory_type)?;
    // User-created memories always have source="conversation" (closest to user input)
    let memory = Memory::new(
        body.content,
        memory_type,
        MemorySource::Conversation,
        body.category,
    );

    let repo = state.memory_repo.clone();
    let saved = repo.save(&memory).await?;

    tracing::info!(
        memory_id = %saved.id,
        category = %saved.category,
        memory_type = %saved.memory_type,
        "memory.created",
    );

    Ok(Json(memory_to_response(&saved)))
}

/// PUT /api/memories/:id
pub async fn update_memory(
    State(state): State<Arc<AppState>>,
    Path(memory_id): Path<Uuid>,
    Json(body): Json<MemoryUpdateRequest>,
) -> Result<Json<MemoryResponse>, AppError> {
    let repo = state.memory_repo.clone();

    repo.get_by_id(memory_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Memory {memory_id} not found")))?;

    let updated = repo
        .update(
            memory_id,
            body.content.as_deref(),
            body.enabled,
            body.category.as_deref(),
        )
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Memory {memory_id} not found")))?;

    tracing::info!(memory_id = %memory_id, "memory.updated");
    Ok(Json(memory_to_response(&updated)))
}

/// DELETE /api/memories/:id
pub async fn delete_memory(
    State(state): State<Arc<AppState>>,
    Path(memory_id): Path<Uuid>,
) -> Result<Json<MemoryActionResponse>, AppError> {
    let repo = state.memory_repo.clone();

    if !repo.delete(memory_id).await? {
        return Err(AppError::NotFound(format!(
            "Memory {memory_id} not found"
        )));
    }

    tracing::info!(memory_id = %memory_id, "memory.deleted");

    Ok(Json(MemoryActionResponse {
        id: memory_id.to_string(),
        enabled: false,
        message: "Memory permanently deleted".into(),
    }))
}

/// POST /api/memories/search
///
/// Semantic search placeholder — requires embedding provider to be wired.
/// For now returns an empty list with a message.
pub async fn search_memories(
    State(_state): State<Arc<AppState>>,
    Json(body): Json<MemorySearchRequest>,
) -> Result<Json<MemoryListResponse>, AppError> {
    if body.query.is_empty() {
        return Err(AppError::Validation("query must not be empty".into()));
    }

    // Full semantic search requires embedding provider; return empty for now
    tracing::info!(query = %body.query, "memory.search_requested");

    Ok(Json(MemoryListResponse {
        memories: vec![],
        count: 0,
        total: 0,
    }))
}
