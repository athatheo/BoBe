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

#[derive(Debug, Serialize)]
pub struct MemorySearchHit {
    pub id: String,
    pub content: String,
    pub memory_type: String,
    pub category: String,
    pub source: String,
    pub enabled: bool,
    pub similarity: f64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct MemorySearchResponse {
    pub results: Vec<MemorySearchHit>,
    pub count: usize,
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
    let limit = params.limit.clamp(1, 1000);
    let offset = params.offset.max(0);

    let (memories, total) = repo
        .find_all(
            params.memory_type.as_deref(),
            params.category.as_deref(),
            params.source.as_deref(),
            params.enabled_only,
            limit,
            offset,
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

/// POST /api/memories/:id/enable
pub async fn enable_memory(
    State(state): State<Arc<AppState>>,
    Path(memory_id): Path<Uuid>,
) -> Result<Json<MemoryActionResponse>, AppError> {
    let repo = state.memory_repo.clone();

    let updated = repo
        .update(memory_id, None, Some(true), None)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Memory {memory_id} not found")))?;

    tracing::info!(memory_id = %memory_id, "memory.enabled");

    Ok(Json(MemoryActionResponse {
        id: updated.id.to_string(),
        enabled: updated.enabled,
        message: "Memory enabled".into(),
    }))
}

/// POST /api/memories/:id/disable
pub async fn disable_memory(
    State(state): State<Arc<AppState>>,
    Path(memory_id): Path<Uuid>,
) -> Result<Json<MemoryActionResponse>, AppError> {
    let repo = state.memory_repo.clone();

    let updated = repo
        .update(memory_id, None, Some(false), None)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Memory {memory_id} not found")))?;

    tracing::info!(memory_id = %memory_id, "memory.disabled");

    Ok(Json(MemoryActionResponse {
        id: updated.id.to_string(),
        enabled: updated.enabled,
        message: "Memory disabled".into(),
    }))
}

/// POST /api/memories/search
pub async fn search_memories(
    State(state): State<Arc<AppState>>,
    Json(body): Json<MemorySearchRequest>,
) -> Result<Json<MemorySearchResponse>, AppError> {
    if body.query.is_empty() {
        return Err(AppError::Validation("query must not be empty".into()));
    }

    let limit = body.limit.clamp(1, 100);

    tracing::info!(query = %body.query, limit, "memory.search_requested");

    let embedding = state.embedding_provider.embed(&body.query).await?;
    let results = state.memory_repo.find_similar(&embedding, limit, true, 0.0).await?;

    let memories: Vec<MemorySearchHit> = results
        .into_iter()
        .map(|(m, score)| MemorySearchHit {
            id: m.id.to_string(),
            content: m.content,
            memory_type: m.memory_type,
            category: m.category,
            source: m.source,
            enabled: m.enabled,
            similarity: score,
            created_at: m.created_at,
            updated_at: m.updated_at,
        })
        .collect();

    let count = memories.len();
    Ok(Json(MemorySearchResponse {
        results: memories,
        count,
    }))
}
