use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::error::AppError;

// ── Schemas ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct SettingsResponse {
    // LLM
    pub llm_backend: String,
    pub ollama_model: String,
    pub openai_model: String,
    pub openai_api_key_set: bool,
    pub azure_openai_endpoint: String,
    pub azure_openai_deployment: String,
    pub azure_openai_api_key_set: bool,

    // Capture
    pub capture_enabled: bool,
    pub capture_interval_seconds: u64,

    // Check-in
    pub checkin_enabled: bool,
    pub checkin_times: Vec<String>,
    pub checkin_jitter_minutes: u32,

    // Learning
    pub learning_enabled: bool,
    pub learning_interval_minutes: u64,

    // Conversation
    pub conversation_inactivity_timeout_seconds: u64,
    pub conversation_auto_close_minutes: u64,
    pub conversation_summary_enabled: bool,

    // Goals
    pub goal_check_interval_seconds: f64,

    // Tools
    pub tools_enabled: bool,
    pub tools_max_iterations: u32,

    // MCP
    pub mcp_enabled: bool,

    // Similarity thresholds
    pub similarity_deduplication_threshold: f64,
    pub similarity_search_recall_threshold: f64,
    pub similarity_clustering_threshold: f64,

    // Memory retention
    pub memory_short_term_retention_days: u32,
    pub memory_long_term_retention_days: u32,
}

#[derive(Debug, Deserialize)]
pub struct SettingsUpdateRequest {
    // LLM
    pub llm_backend: Option<String>,
    pub ollama_model: Option<String>,
    pub openai_model: Option<String>,
    pub openai_api_key: Option<String>,
    pub azure_openai_endpoint: Option<String>,
    pub azure_openai_deployment: Option<String>,
    pub azure_openai_api_key: Option<String>,

    // Capture
    pub capture_enabled: Option<bool>,
    pub capture_interval_seconds: Option<u64>,

    // Check-in
    pub checkin_enabled: Option<bool>,
    pub checkin_times: Option<Vec<String>>,
    pub checkin_jitter_minutes: Option<u32>,

    // Learning
    pub learning_enabled: Option<bool>,
    pub learning_interval_minutes: Option<u64>,

    // Conversation
    pub conversation_inactivity_timeout_seconds: Option<u64>,
    pub conversation_auto_close_minutes: Option<u64>,
    pub conversation_summary_enabled: Option<bool>,

    // Goals
    pub goal_check_interval_seconds: Option<f64>,

    // Tools
    pub tools_enabled: Option<bool>,
    pub tools_max_iterations: Option<u32>,

    // MCP
    pub mcp_enabled: Option<bool>,

    // Similarity thresholds
    pub similarity_deduplication_threshold: Option<f64>,
    pub similarity_search_recall_threshold: Option<f64>,
    pub similarity_clustering_threshold: Option<f64>,

    // Memory retention
    pub memory_short_term_retention_days: Option<u32>,
    pub memory_long_term_retention_days: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct SettingsUpdateResponse {
    pub message: String,
    pub applied_fields: Vec<String>,
    pub restart_required_fields: Vec<String>,
}

// ── Handlers ────────────────────────────────────────────────────────────────

/// GET /api/settings
pub async fn get_settings(
    State(state): State<Arc<AppState>>,
) -> Result<Json<SettingsResponse>, AppError> {
    let cfg = state.config();

    Ok(Json(SettingsResponse {
        llm_backend: cfg.llm_backend.clone(),
        ollama_model: cfg.ollama_model.clone(),
        openai_model: cfg.openai_model.clone(),
        openai_api_key_set: !cfg.openai_api_key.is_empty(),
        azure_openai_endpoint: cfg.azure_openai_endpoint.clone(),
        azure_openai_deployment: cfg.azure_openai_deployment.clone(),
        azure_openai_api_key_set: !cfg.azure_openai_api_key.is_empty(),
        capture_enabled: cfg.capture_enabled,
        capture_interval_seconds: cfg.capture_interval_seconds,
        checkin_enabled: cfg.checkin_enabled,
        checkin_times: cfg.checkin_times_vec(),
        checkin_jitter_minutes: cfg.checkin_jitter_minutes,
        learning_enabled: cfg.learning_enabled,
        learning_interval_minutes: cfg.learning_interval_minutes,
        conversation_inactivity_timeout_seconds: cfg.conversation_inactivity_timeout_seconds,
        conversation_auto_close_minutes: cfg.conversation_auto_close_minutes,
        conversation_summary_enabled: cfg.conversation_summary_enabled,
        goal_check_interval_seconds: cfg.goal_check_interval_seconds,
        tools_enabled: cfg.tools_enabled,
        tools_max_iterations: cfg.tools_max_iterations,
        mcp_enabled: cfg.mcp_enabled,
        similarity_deduplication_threshold: cfg.similarity_deduplication_threshold,
        similarity_search_recall_threshold: cfg.similarity_search_recall_threshold,
        similarity_clustering_threshold: cfg.similarity_clustering_threshold,
        memory_short_term_retention_days: cfg.memory_short_term_retention_days,
        memory_long_term_retention_days: cfg.memory_long_term_retention_days,
    }))
}

/// PUT /api/settings
///
/// Hot-swaps configuration at runtime via ArcSwap. Most settings take effect
/// immediately without restart.
pub async fn update_settings(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SettingsUpdateRequest>,
) -> Result<Json<SettingsUpdateResponse>, AppError> {
    let current = state.config.load();
    let mut new_config = (**current).clone();
    let mut applied = Vec::new();

    // Apply each optional field
    if let Some(ref v) = body.llm_backend {
        new_config.llm_backend = v.clone();
        applied.push("llm_backend".into());
    }
    if let Some(ref v) = body.ollama_model {
        new_config.ollama_model = v.clone();
        applied.push("ollama_model".into());
    }
    if let Some(ref v) = body.openai_model {
        new_config.openai_model = v.clone();
        applied.push("openai_model".into());
    }
    if let Some(ref v) = body.openai_api_key {
        new_config.openai_api_key = v.clone();
        applied.push("openai_api_key".into());
    }
    if let Some(ref v) = body.azure_openai_endpoint {
        new_config.azure_openai_endpoint = v.clone();
        applied.push("azure_openai_endpoint".into());
    }
    if let Some(ref v) = body.azure_openai_deployment {
        new_config.azure_openai_deployment = v.clone();
        applied.push("azure_openai_deployment".into());
    }
    if let Some(ref v) = body.azure_openai_api_key {
        new_config.azure_openai_api_key = v.clone();
        applied.push("azure_openai_api_key".into());
    }
    if let Some(v) = body.capture_enabled {
        new_config.capture_enabled = v;
        applied.push("capture_enabled".into());
    }
    if let Some(v) = body.capture_interval_seconds {
        new_config.capture_interval_seconds = v;
        applied.push("capture_interval_seconds".into());
    }
    if let Some(v) = body.checkin_enabled {
        new_config.checkin_enabled = v;
        applied.push("checkin_enabled".into());
    }
    if let Some(ref v) = body.checkin_times {
        new_config.checkin_times = v.join(",");
        applied.push("checkin_times".into());
    }
    if let Some(v) = body.checkin_jitter_minutes {
        new_config.checkin_jitter_minutes = v;
        applied.push("checkin_jitter_minutes".into());
    }
    if let Some(v) = body.learning_enabled {
        new_config.learning_enabled = v;
        applied.push("learning_enabled".into());
    }
    if let Some(v) = body.learning_interval_minutes {
        new_config.learning_interval_minutes = v;
        applied.push("learning_interval_minutes".into());
    }
    if let Some(v) = body.conversation_inactivity_timeout_seconds {
        new_config.conversation_inactivity_timeout_seconds = v;
        applied.push("conversation_inactivity_timeout_seconds".into());
    }
    if let Some(v) = body.conversation_auto_close_minutes {
        new_config.conversation_auto_close_minutes = v;
        applied.push("conversation_auto_close_minutes".into());
    }
    if let Some(v) = body.conversation_summary_enabled {
        new_config.conversation_summary_enabled = v;
        applied.push("conversation_summary_enabled".into());
    }
    if let Some(v) = body.goal_check_interval_seconds {
        new_config.goal_check_interval_seconds = v;
        applied.push("goal_check_interval_seconds".into());
    }
    if let Some(v) = body.tools_enabled {
        new_config.tools_enabled = v;
        applied.push("tools_enabled".into());
    }
    if let Some(v) = body.tools_max_iterations {
        new_config.tools_max_iterations = v;
        applied.push("tools_max_iterations".into());
    }
    if let Some(v) = body.mcp_enabled {
        new_config.mcp_enabled = v;
        applied.push("mcp_enabled".into());
    }
    if let Some(v) = body.similarity_deduplication_threshold {
        new_config.similarity_deduplication_threshold = v;
        applied.push("similarity_deduplication_threshold".into());
    }
    if let Some(v) = body.similarity_search_recall_threshold {
        new_config.similarity_search_recall_threshold = v;
        applied.push("similarity_search_recall_threshold".into());
    }
    if let Some(v) = body.similarity_clustering_threshold {
        new_config.similarity_clustering_threshold = v;
        applied.push("similarity_clustering_threshold".into());
    }
    if let Some(v) = body.memory_short_term_retention_days {
        new_config.memory_short_term_retention_days = v;
        applied.push("memory_short_term_retention_days".into());
    }
    if let Some(v) = body.memory_long_term_retention_days {
        new_config.memory_long_term_retention_days = v;
        applied.push("memory_long_term_retention_days".into());
    }

    // Hot-swap the config
    state.config.store(Arc::new(new_config));

    tracing::info!(fields = ?applied, "settings.updated");

    Ok(Json(SettingsUpdateResponse {
        message: format!("Updated {} setting(s)", applied.len()),
        applied_fields: applied,
        restart_required_fields: vec![],
    }))
}
