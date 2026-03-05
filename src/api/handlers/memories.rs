use std::sync::Arc;

use crate::app_state::AppState;
use crate::constants::{MEMORY_CONTENT_MAX_LENGTH, MEMORY_CONTENT_MIN_LENGTH};
use crate::db::MemoryRepository;
use crate::error::AppError;
use crate::models::ids::MemoryId;
use crate::models::memory::Memory;
use crate::models::types::{MemorySource, MemoryType};
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub(crate) struct MemoryResponse {
    pub(crate) id: String,
    pub(crate) content: String,
    pub(crate) memory_type: String,
    pub(crate) category: String,
    pub(crate) source: String,
    pub(crate) enabled: bool,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub(crate) struct MemoryListResponse {
    pub(crate) memories: Vec<MemoryResponse>,
    pub(crate) count: usize,
    pub(crate) total: i64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MemoryCreateRequest {
    pub(crate) content: String,
    #[serde(default = "default_category")]
    pub(crate) category: String,
    #[serde(default = "default_memory_type")]
    pub(crate) memory_type: String,
}

fn default_category() -> String {
    "general".into()
}

fn default_memory_type() -> String {
    "explicit".into()
}

#[derive(Debug, Deserialize)]
pub(crate) struct MemoryUpdateRequest {
    pub(crate) content: Option<String>,
    pub(crate) category: Option<String>,
    pub(crate) enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
pub(crate) struct MemoryActionResponse {
    pub(crate) id: String,
    pub(crate) enabled: bool,
    pub(crate) message: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MemoryListQuery {
    pub(crate) memory_type: Option<MemoryType>,
    pub(crate) category: Option<String>,
    pub(crate) source: Option<MemorySource>,
    #[serde(default)]
    pub(crate) enabled_only: bool,
    #[serde(default = "default_limit")]
    pub(crate) limit: i64,
    #[serde(default)]
    pub(crate) offset: i64,
}

fn default_limit() -> i64 {
    100
}

#[derive(Debug, Deserialize)]
pub(crate) struct MemorySearchRequest {
    pub(crate) query: String,
    #[serde(default = "default_search_limit")]
    pub(crate) limit: i64,
}

fn default_search_limit() -> i64 {
    20
}

#[derive(Debug, Serialize)]
pub(crate) struct MemorySearchHit {
    pub(crate) id: String,
    pub(crate) content: String,
    pub(crate) memory_type: String,
    pub(crate) category: String,
    pub(crate) source: String,
    pub(crate) enabled: bool,
    pub(crate) similarity: f64,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub(crate) struct MemorySearchResponse {
    pub(crate) results: Vec<MemorySearchHit>,
    pub(crate) count: usize,
}

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
    memory_id: MemoryId,
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

pub(crate) async fn list_memories(
    State(state): State<Arc<AppState>>,
    Query(params): Query<MemoryListQuery>,
) -> Result<Json<MemoryListResponse>, AppError> {
    let limit = params.limit.clamp(1, 1000);
    let offset = params.offset.max(0);

    let (memories, total) = state
        .memory_repo
        .find_all(
            params.memory_type,
            params.category.as_deref(),
            params.source,
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

pub(crate) async fn get_memory(
    State(state): State<Arc<AppState>>,
    Path(memory_id): Path<MemoryId>,
) -> Result<Json<MemoryResponse>, AppError> {
    let memory = state
        .memory_repo
        .get_by_id(memory_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Memory {memory_id} not found")))?;

    Ok(Json(memory_to_response(&memory)))
}

pub(crate) async fn create_memory(
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

pub(crate) async fn update_memory(
    State(state): State<Arc<AppState>>,
    Path(memory_id): Path<MemoryId>,
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

pub(crate) async fn delete_memory(
    State(state): State<Arc<AppState>>,
    Path(memory_id): Path<MemoryId>,
) -> Result<StatusCode, AppError> {
    if !state.memory_repo.delete(memory_id).await? {
        return Err(AppError::NotFound(format!("Memory {memory_id} not found")));
    }

    tracing::info!(memory_id = %memory_id, "memory.deleted");

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn enable_memory(
    State(state): State<Arc<AppState>>,
    Path(memory_id): Path<MemoryId>,
) -> Result<Json<MemoryActionResponse>, AppError> {
    set_memory_enabled(&state.memory_repo, memory_id, true).await
}

pub(crate) async fn disable_memory(
    State(state): State<Arc<AppState>>,
    Path(memory_id): Path<MemoryId>,
) -> Result<Json<MemoryActionResponse>, AppError> {
    set_memory_enabled(&state.memory_repo, memory_id, false).await
}

pub(crate) async fn search_memories(
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
