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
    pub projects_directory: String,
    pub goal_worker_enabled: bool,
    pub goal_worker_autonomous: bool,
    pub goal_worker_max_concurrent: u32,

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
    pub projects_directory: Option<String>,
    pub goal_worker_enabled: Option<bool>,
    pub goal_worker_autonomous: Option<bool>,
    pub goal_worker_max_concurrent: Option<u32>,

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
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub persist_failed: bool,
}

// ── Handlers ────────────────────────────────────────────────────────────────

/// GET /api/settings
pub async fn get_settings(
    State(state): State<Arc<AppState>>,
) -> Result<Json<SettingsResponse>, AppError> {
    let cfg = state.config();

    Ok(Json(SettingsResponse {
        llm_backend: cfg.llm.backend,
        ollama_model: cfg.ollama.model.clone(),
        openai_model: cfg.llm.openai_model.clone(),
        openai_api_key_set: !cfg.llm.openai_api_key.is_empty(),
        azure_openai_endpoint: cfg.llm.azure_openai_endpoint.clone(),
        azure_openai_deployment: cfg.llm.azure_openai_deployment.clone(),
        azure_openai_api_key_set: !cfg.llm.azure_openai_api_key.is_empty(),
        vision_backend: cfg.vision.backend,
        vision_ollama_model: cfg.vision.ollama_model.clone(),
        capture_enabled: cfg.capture.enabled,
        capture_interval_seconds: cfg.capture.interval_seconds,
        checkin_enabled: cfg.checkin.enabled,
        checkin_times: cfg.checkin_times_vec().to_vec(),
        checkin_jitter_minutes: cfg.checkin.jitter_minutes,
        learning_enabled: cfg.learning.enabled,
        learning_interval_minutes: cfg.learning.interval_minutes,
        conversation_inactivity_timeout_seconds: cfg.conversation.inactivity_timeout_seconds,
        conversation_auto_close_minutes: cfg.conversation.auto_close_minutes,
        conversation_summary_enabled: cfg.conversation.summary_enabled,
        goal_check_interval_seconds: cfg.goals.check_interval_seconds,
        projects_directory: cfg.resolved_projects_dir().to_string_lossy().to_string(),
        goal_worker_enabled: cfg.goal_worker.enabled,
        goal_worker_autonomous: cfg.goal_worker.autonomous,
        goal_worker_max_concurrent: cfg.goal_worker.max_concurrent,
        tools_enabled: cfg.tools.enabled,
        tools_max_iterations: cfg.tools.max_iterations,
        mcp_enabled: cfg.mcp.enabled,
        similarity_deduplication_threshold: cfg.similarity.deduplication_threshold,
        similarity_search_recall_threshold: cfg.similarity.search_recall_threshold,
        similarity_clustering_threshold: cfg.similarity.clustering_threshold,
        memory_short_term_retention_days: cfg.memory.short_term_retention_days,
        memory_long_term_retention_days: cfg.memory.long_term_retention_days,
    }))
}

/// PATCH /api/settings
///
/// Hot-swaps configuration at runtime through ConfigManager.
/// Changes are persisted to ~/.bobe/config.toml and LLM provider is rebuilt
/// when backend/model/key fields change.
pub async fn update_settings(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SettingsUpdateRequest>,
) -> Result<Json<SettingsUpdateResponse>, AppError> {
    // Validate llm_backend early (before passing to ConfigManager).
    if let Some(ref v) = body.llm_backend {
        let _: LlmBackend =
            serde_json::from_value(serde_json::Value::String(v.clone())).map_err(|_| {
                AppError::Validation(format!(
                    "Invalid llm_backend '{v}'. Valid: ollama, openai, azure_openai, llamacpp"
                ))
            })?;
    }

    let llm_change_requested = body.llm_backend.is_some()
        || body.ollama_model.is_some()
        || body.openai_model.is_some()
        || body.openai_api_key.is_some()
        || body.azure_openai_endpoint.is_some()
        || body.azure_openai_deployment.is_some()
        || body.azure_openai_api_key.is_some();

    if llm_change_requested {
        validate_llm_settings_update(&state, &body).await?;
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
    collect_opt!(goal_worker_enabled);
    collect_opt!(goal_worker_autonomous);
    collect_opt!(goal_worker_max_concurrent);
    collect_opt!(tools_enabled);
    collect_opt!(tools_max_iterations);
    collect_opt!(mcp_enabled);
    collect_opt!(similarity_deduplication_threshold);
    collect_opt!(similarity_search_recall_threshold);
    collect_opt!(similarity_clustering_threshold);
    collect_opt!(memory_short_term_retention_days);
    collect_opt!(memory_long_term_retention_days);

    // checkin_times uses nested key to avoid flat legacy conversion.
    if let Some(ref v) = body.checkin_times {
        changes.insert(
            "checkin.times".to_owned(),
            serde_json::to_value(v).unwrap_or_default(),
        );
    }

    // Keep frontend field name while writing canonical config key.
    if let Some(ref v) = body.projects_directory {
        changes.insert(
            "goal_worker.projects_dir".to_owned(),
            serde_json::Value::String(v.clone()),
        );
    }

    if changes.is_empty() {
        return Ok(Json(SettingsUpdateResponse {
            message: "No changes provided".into(),
            applied_fields: vec![],
            restart_required_fields: vec![],
            persist_failed: false,
        }));
    }

    // Route through ConfigManager: persists to config.toml, swaps config, rebuilds LLM.
    let result = state.config_manager.update(&changes);

    tracing::info!(
        applied = ?result.applied_fields,
        restart_required = ?result.restart_required_fields,
        persist_failed = result.persist_failed,
        "settings.updated"
    );

    let total = result.applied_fields.len() + result.restart_required_fields.len();
    Ok(Json(SettingsUpdateResponse {
        message: format!("Updated {total} setting(s)"),
        applied_fields: result.applied_fields,
        restart_required_fields: result.restart_required_fields,
        persist_failed: result.persist_failed,
    }))
}

async fn validate_llm_settings_update(
    state: &Arc<AppState>,
    body: &SettingsUpdateRequest,
) -> Result<(), AppError> {
    let cfg = state.config();
    let backend = if let Some(ref v) = body.llm_backend {
        serde_json::from_value(serde_json::Value::String(v.clone()))
            .map_err(|_| AppError::Validation(format!("Invalid llm_backend '{v}'")))?
    } else {
        cfg.llm.backend
    };

    let ollama_url = cfg.ollama.url.clone();
    let llama_url = cfg.llm.llama_url.clone();
    let openai_key = body
        .openai_api_key
        .clone()
        .unwrap_or_else(|| cfg.llm.openai_api_key.clone());
    let azure_endpoint = body
        .azure_openai_endpoint
        .clone()
        .unwrap_or_else(|| cfg.llm.azure_openai_endpoint.clone());
    let azure_key = body
        .azure_openai_api_key
        .clone()
        .unwrap_or_else(|| cfg.llm.azure_openai_api_key.clone());
    let azure_deployment = body
        .azure_openai_deployment
        .clone()
        .unwrap_or_else(|| cfg.llm.azure_openai_deployment.clone());
    drop(cfg);

    let client = &state.http_client;
    match backend {
        LlmBackend::Ollama => {
            let url = format!("{}/api/tags", ollama_url.trim_end_matches('/'));
            let resp = client
                .get(&url)
                .send()
                .await
                .map_err(|e| AppError::Validation(format!("Cannot reach Ollama: {e}")))?;
            if !resp.status().is_success() {
                return Err(AppError::Validation(format!(
                    "Ollama validation failed: HTTP {}",
                    resp.status()
                )));
            }
        }
        LlmBackend::Openai => {
            if openai_key.trim().is_empty() {
                return Err(AppError::Validation(
                    "OpenAI API key is required for openai backend".into(),
                ));
            }
            let resp = client
                .get("https://api.openai.com/v1/models")
                .header("Authorization", format!("Bearer {openai_key}"))
                .send()
                .await
                .map_err(|e| AppError::Validation(format!("Cannot reach OpenAI: {e}")))?;
            if !resp.status().is_success() {
                return Err(AppError::Validation(format!(
                    "OpenAI validation failed: HTTP {}",
                    resp.status()
                )));
            }
        }
        LlmBackend::AzureOpenai => {
            if azure_endpoint.trim().is_empty()
                || azure_key.trim().is_empty()
                || azure_deployment.trim().is_empty()
            {
                return Err(AppError::Validation(
                    "Azure OpenAI endpoint, API key, and deployment are required".into(),
                ));
            }
            let test_url = format!(
                "{}/openai/deployments/{}/chat/completions?api-version=2024-02-15-preview",
                azure_endpoint.trim_end_matches('/'),
                azure_deployment
            );
            let resp = client
                .post(&test_url)
                .header("api-key", &azure_key)
                .json(&serde_json::json!({"messages": [{"role": "user", "content": "test"}], "max_tokens": 1}))
                .send()
                .await
                .map_err(|e| AppError::Validation(format!("Cannot reach Azure endpoint: {e}")))?;
            if !resp.status().is_success() && resp.status().as_u16() != 400 {
                return Err(AppError::Validation(format!(
                    "Azure validation failed: HTTP {}",
                    resp.status()
                )));
            }
        }
        LlmBackend::LlamaCpp => {
            let url = format!("{}/v1/models", llama_url.trim_end_matches('/'));
            let resp =
                client.get(&url).send().await.map_err(|e| {
                    AppError::Validation(format!("Cannot reach llama.cpp server: {e}"))
                })?;
            if !resp.status().is_success() {
                return Err(AppError::Validation(format!(
                    "llama.cpp validation failed: HTTP {}",
                    resp.status()
                )));
            }
        }
        LlmBackend::None => {
            return Err(AppError::Validation(
                "llm backend cannot be 'none' for runtime settings".into(),
            ));
        }
    }

    Ok(())
}
