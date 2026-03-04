use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::constants::{MEMORY_CONTENT_MAX_LENGTH, MEMORY_CONTENT_MIN_LENGTH};
use crate::db::MemoryRepository;
use crate::error::AppError;
use crate::models::memory::Memory;
use crate::models::types::{MemorySource, MemoryType};

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
        memory_type: memory.memory_type.as_str().to_owned(),
        category: memory.category.clone(),
        source: memory.source.as_str().to_owned(),
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

async fn set_memory_enabled(
    memory_repo: &Arc<dyn MemoryRepository>,
    memory_id: Uuid,
    enabled: bool,
) -> Result<Json<MemoryActionResponse>, AppError> {
    let updated = memory_repo
        .update(memory_id, None, Some(enabled), None)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Memory {memory_id} not found")))?;

    if enabled {
        tracing::info!(memory_id = %memory_id, "memory.enabled");
    } else {
        tracing::info!(memory_id = %memory_id, "memory.disabled");
    }

    Ok(Json(MemoryActionResponse {
        id: updated.id.to_string(),
        enabled: updated.enabled,
        message: if enabled {
            "Memory enabled".into()
        } else {
            "Memory disabled".into()
        },
    }))
}

// ── Handlers ────────────────────────────────────────────────────────────────

/// GET /api/memories
pub async fn list_memories(
    State(state): State<Arc<AppState>>,
    Query(params): Query<MemoryListQuery>,
) -> Result<Json<MemoryListResponse>, AppError> {
    let limit = params.limit.clamp(1, 1000);
    let offset = params.offset.max(0);

    let (memories, total) = state
        .memory_repo
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
    let memory = state
        .memory_repo
        .get_by_id(memory_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Memory {memory_id} not found")))?;

    Ok(Json(memory_to_response(&memory)))
}

/// POST /api/memories
pub async fn create_memory(
    State(state): State<Arc<AppState>>,
    Json(body): Json<MemoryCreateRequest>,
) -> Result<(StatusCode, Json<MemoryResponse>), AppError> {
    if body.content.len() < MEMORY_CONTENT_MIN_LENGTH
        || body.content.len() > MEMORY_CONTENT_MAX_LENGTH
    {
        return Err(AppError::Validation(format!(
            "content must be between {MEMORY_CONTENT_MIN_LENGTH} and {MEMORY_CONTENT_MAX_LENGTH} characters"
        )));
    }

    let memory_type = parse_memory_type(&body.memory_type)?;
    // User-created memories always have source="conversation" (closest to user input)
    let memory = Memory::new(
        body.content,
        memory_type,
        MemorySource::Conversation,
        body.category,
    );

    let saved = state.memory_repo.save(&memory).await?;

    tracing::info!(
        memory_id = %saved.id,
        category = %saved.category,
        memory_type = %saved.memory_type,
        "memory.created",
    );

    Ok((StatusCode::CREATED, Json(memory_to_response(&saved))))
}

/// PUT /api/memories/:id
pub async fn update_memory(
    State(state): State<Arc<AppState>>,
    Path(memory_id): Path<Uuid>,
    Json(body): Json<MemoryUpdateRequest>,
) -> Result<Json<MemoryResponse>, AppError> {
    state
        .memory_repo
        .get_by_id(memory_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Memory {memory_id} not found")))?;

    let updated = state
        .memory_repo
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
) -> Result<StatusCode, AppError> {
    if !state.memory_repo.delete(memory_id).await? {
        return Err(AppError::NotFound(format!("Memory {memory_id} not found")));
    }

    tracing::info!(memory_id = %memory_id, "memory.deleted");

    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/memories/:id/enable
pub async fn enable_memory(
    State(state): State<Arc<AppState>>,
    Path(memory_id): Path<Uuid>,
) -> Result<Json<MemoryActionResponse>, AppError> {
    set_memory_enabled(&state.memory_repo, memory_id, true).await
}

/// POST /api/memories/:id/disable
pub async fn disable_memory(
    State(state): State<Arc<AppState>>,
    Path(memory_id): Path<Uuid>,
) -> Result<Json<MemoryActionResponse>, AppError> {
    set_memory_enabled(&state.memory_repo, memory_id, false).await
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
    let results = state
        .memory_repo
        .find_similar(&embedding, limit, true, 0.0)
        .await?;

    let memories: Vec<MemorySearchHit> = results
        .into_iter()
        .map(|(m, score)| MemorySearchHit {
            id: m.id.to_string(),
            content: m.content,
            memory_type: m.memory_type.as_str().to_owned(),
            category: m.category,
            source: m.source.as_str().to_owned(),
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
