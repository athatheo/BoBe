use std::collections::HashMap;
use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::config::LlmBackend;
use crate::error::AppError;

// ── Schemas ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct SettingsResponse {
    // LLM
    pub llm_backend: LlmBackend,
    pub ollama_model: String,
    pub openai_model: String,
    pub openai_api_key_set: bool,
    pub azure_openai_endpoint: String,
    pub azure_openai_deployment: String,
    pub azure_openai_api_key_set: bool,

    // Vision
    pub vision_backend: LlmBackend,
    pub vision_ollama_model: String,

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

    // Vision
    pub vision_backend: Option<String>,
    pub vision_ollama_model: Option<String>,

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
        llm_backend: cfg.llm_backend,
        ollama_model: cfg.ollama_model.clone(),
        openai_model: cfg.openai_model.clone(),
        openai_api_key_set: !cfg.openai_api_key.is_empty(),
        azure_openai_endpoint: cfg.azure_openai_endpoint.clone(),
        azure_openai_deployment: cfg.azure_openai_deployment.clone(),
        azure_openai_api_key_set: !cfg.azure_openai_api_key.is_empty(),
        vision_backend: cfg.vision_backend,
        vision_ollama_model: cfg.vision_ollama_model.clone(),
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
/// Hot-swaps configuration at runtime through ConfigManager.
/// Changes are persisted to ~/.bobe/.env and LLM provider is rebuilt
/// when backend/model/key fields change.
pub async fn update_settings(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SettingsUpdateRequest>,
) -> Result<Json<SettingsUpdateResponse>, AppError> {
    // Validate llm_backend early (before passing to ConfigManager).
    if let Some(ref v) = body.llm_backend {
        let _: LlmBackend = serde_json::from_value(serde_json::Value::String(v.clone()))
            .map_err(|_| {
                AppError::Validation(format!(
                    "Invalid llm_backend '{v}'. Valid: ollama, openai, azure_openai, llamacpp"
                ))
            })?;
    }

    // Collect all provided fields into a HashMap for ConfigManager.
    let mut changes: HashMap<String, serde_json::Value> = HashMap::new();

    macro_rules! collect_opt {
        ($field:ident) => {
            if let Some(ref v) = body.$field {
                changes.insert(
                    stringify!($field).to_owned(),
                    serde_json::to_value(v).unwrap_or_default(),
                );
            }
        };
    }

    collect_opt!(llm_backend);
    collect_opt!(ollama_model);
    collect_opt!(openai_model);
    collect_opt!(openai_api_key);
    collect_opt!(azure_openai_endpoint);
    collect_opt!(azure_openai_deployment);
    collect_opt!(azure_openai_api_key);
    collect_opt!(vision_backend);
    collect_opt!(vision_ollama_model);
    collect_opt!(capture_enabled);
    collect_opt!(capture_interval_seconds);
    collect_opt!(checkin_enabled);
    collect_opt!(checkin_jitter_minutes);
    collect_opt!(learning_enabled);
    collect_opt!(learning_interval_minutes);
    collect_opt!(conversation_inactivity_timeout_seconds);
    collect_opt!(conversation_auto_close_minutes);
    collect_opt!(conversation_summary_enabled);
    collect_opt!(goal_check_interval_seconds);
    collect_opt!(tools_enabled);
    collect_opt!(tools_max_iterations);
    collect_opt!(mcp_enabled);
    collect_opt!(similarity_deduplication_threshold);
    collect_opt!(similarity_search_recall_threshold);
    collect_opt!(similarity_clustering_threshold);
    collect_opt!(memory_short_term_retention_days);
    collect_opt!(memory_long_term_retention_days);

    // checkin_times needs special handling (Vec<String> -> comma-separated)
    if let Some(ref v) = body.checkin_times {
        changes.insert(
            "checkin_times".to_owned(),
            serde_json::Value::String(v.join(",")),
        );
    }

    if changes.is_empty() {
        return Ok(Json(SettingsUpdateResponse {
            message: "No changes provided".into(),
            applied_fields: vec![],
            restart_required_fields: vec![],
        }));
    }

    // Route through ConfigManager: persists to .env, swaps config, rebuilds LLM.
    let result = state.config_manager.update(&changes);

    tracing::info!(
        applied = ?result.applied_fields,
        restart_required = ?result.restart_required_fields,
        persist_failed = result.persist_failed,
        "settings.updated"
    );

    let total = result.applied_fields.len() + result.restart_required_fields.len();
    Ok(Json(SettingsUpdateResponse {
        message: format!("Updated {} setting(s)", total),
        applied_fields: result.applied_fields,
        restart_required_fields: result.restart_required_fields,
    }))
}
