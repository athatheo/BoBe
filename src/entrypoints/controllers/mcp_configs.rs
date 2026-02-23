use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;


use crate::adapters::tools::mcp::adapter::db_config_to_parsed;
use crate::app_state::AppState;
use crate::domain::mcp_server_config::McpServerConfig;
use crate::error::AppError;


// ── Schemas ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct McpConfigResponse {
    pub id: String,
    pub server_name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub enabled: bool,
    pub timeout_seconds: f64,
    pub is_default: bool,
    pub excluded_tools: Vec<String>,
    pub last_connected_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct McpConfigListResponse {
    pub configs: Vec<McpConfigResponse>,
    pub count: usize,
    pub enabled_count: usize,
}

#[derive(Debug, Deserialize)]
pub struct McpConfigCreateRequest {
    pub server_name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: f64,
    #[serde(default)]
    pub excluded_tools: Vec<String>,
}

fn default_true() -> bool {
    true
}

fn default_timeout() -> f64 {
    30.0
}

#[derive(Debug, Deserialize)]
pub struct McpConfigUpdateRequest {
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub enabled: Option<bool>,
    pub timeout_seconds: Option<f64>,
    pub excluded_tools: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct McpConfigUpdateResponse {
    pub id: String,
    pub server_name: String,
    pub enabled: bool,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct McpConfigStatusResponse {
    pub id: String,
    pub server_name: String,
    pub status: String,
    pub message: String,
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn config_to_response(cfg: &McpServerConfig) -> McpConfigResponse {
    let excluded: Vec<String> = cfg
        .excluded_tools
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    McpConfigResponse {
        id: cfg.id.to_string(),
        server_name: cfg.server_name.clone(),
        command: cfg.command.clone(),
        args: cfg.args_vec(),
        env: cfg.env_map(),
        enabled: cfg.enabled,
        timeout_seconds: cfg.timeout_seconds,
        is_default: cfg.is_default,
        excluded_tools: excluded,
        last_connected_at: cfg.last_connected_at,
        last_error: cfg.last_error.clone(),
        created_at: cfg.created_at,
        updated_at: cfg.updated_at,
    }
}

// ── Handlers ────────────────────────────────────────────────────────────────

/// GET /api/mcp-configs
pub async fn list_configs(
    State(state): State<Arc<AppState>>,
) -> Result<Json<McpConfigListResponse>, AppError> {
    let configs = state.mcp_config_repo.get_all().await?;
    let enabled_count = configs.iter().filter(|c| c.enabled).count();

    Ok(Json(McpConfigListResponse {
        count: configs.len(),
        enabled_count,
        configs: configs.iter().map(config_to_response).collect(),
    }))
}

/// POST /api/mcp-configs
pub async fn create_config(
    State(state): State<Arc<AppState>>,
    Json(body): Json<McpConfigCreateRequest>,
) -> Result<(StatusCode, Json<McpConfigResponse>), AppError> {
    if body.server_name.is_empty() {
        return Err(AppError::Validation(
            "server_name must not be empty".into(),
        ));
    }
    if body.command.is_empty() {
        return Err(AppError::Validation("command must not be empty".into()));
    }

    if state.mcp_config_repo.get_by_name(&body.server_name).await?.is_some() {
        return Err(AppError::Validation(format!(
            "MCP config with name '{}' already exists",
            body.server_name
        )));
    }

    let args_json = serde_json::to_string(&body.args)?;
    let env_json = serde_json::to_string(&body.env)?;
    let excluded_json = serde_json::to_string(&body.excluded_tools)?;

    let mut cfg = McpServerConfig::new(body.server_name, body.command);
    cfg.args = args_json;
    cfg.env = env_json;
    cfg.enabled = body.enabled;
    cfg.timeout_seconds = body.timeout_seconds;
    cfg.excluded_tools = Some(excluded_json);

    let saved = state.mcp_config_repo.save(&cfg).await?;

    tracing::info!(
        config_id = %saved.id,
        name = %saved.server_name,
        "mcp_config.created",
    );

    Ok((StatusCode::CREATED, Json(config_to_response(&saved))))
}

/// PUT /api/mcp-configs/:id
pub async fn update_config(
    State(state): State<Arc<AppState>>,
    Path(config_id): Path<Uuid>,
    Json(body): Json<McpConfigUpdateRequest>,
) -> Result<Json<McpConfigUpdateResponse>, AppError> {
    let _existing = state.mcp_config_repo
        .get_by_id(config_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("MCP config {config_id} not found")))?;

    let args_json = body
        .args
        .as_ref()
        .map(serde_json::to_string)
        .transpose()?;
    let env_json = body
        .env
        .as_ref()
        .map(serde_json::to_string)
        .transpose()?;
    let excluded_json = body
        .excluded_tools
        .as_ref()
        .map(serde_json::to_string)
        .transpose()?;

    let updated = state.mcp_config_repo
        .update(
            config_id,
            body.command.as_deref(),
            args_json.as_deref(),
            env_json.as_deref(),
            body.enabled,
            body.timeout_seconds,
            excluded_json.as_deref(),
        )
        .await?
        .ok_or_else(|| AppError::NotFound(format!("MCP config {config_id} not found")))?;

    tracing::info!(config_id = %config_id, "mcp_config.updated");

    Ok(Json(McpConfigUpdateResponse {
        id: config_id.to_string(),
        server_name: updated.server_name,
        enabled: updated.enabled,
        message: "MCP config updated".into(),
    }))
}

/// GET /api/mcp-configs/:id
pub async fn get_config(
    State(state): State<Arc<AppState>>,
    Path(config_id): Path<Uuid>,
) -> Result<Json<McpConfigResponse>, AppError> {
    let cfg = state.mcp_config_repo
        .get_by_id(config_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("MCP config {config_id} not found")))?;

    Ok(Json(config_to_response(&cfg)))
}

/// POST /api/mcp-configs/:id/enable
pub async fn enable_config(
    State(state): State<Arc<AppState>>,
    Path(config_id): Path<Uuid>,
) -> Result<Json<McpConfigUpdateResponse>, AppError> {
    let updated = state.mcp_config_repo
        .update(config_id, None, None, None, Some(true), None, None)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("MCP config {config_id} not found")))?;

    tracing::info!(config_id = %config_id, name = %updated.server_name, "mcp_config.enabled");

    Ok(Json(McpConfigUpdateResponse {
        id: config_id.to_string(),
        server_name: updated.server_name,
        enabled: true,
        message: "MCP config enabled".into(),
    }))
}

/// POST /api/mcp-configs/:id/disable
pub async fn disable_config(
    State(state): State<Arc<AppState>>,
    Path(config_id): Path<Uuid>,
) -> Result<Json<McpConfigUpdateResponse>, AppError> {
    let updated = state.mcp_config_repo
        .update(config_id, None, None, None, Some(false), None, None)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("MCP config {config_id} not found")))?;

    tracing::info!(config_id = %config_id, name = %updated.server_name, "mcp_config.disabled");

    Ok(Json(McpConfigUpdateResponse {
        id: config_id.to_string(),
        server_name: updated.server_name,
        enabled: false,
        message: "MCP config disabled".into(),
    }))
}

/// POST /api/mcp-configs/:id/connect
pub async fn connect_server(
    State(state): State<Arc<AppState>>,
    Path(config_id): Path<Uuid>,
) -> Result<Json<McpConfigStatusResponse>, AppError> {
    let mcp_adapter = state
        .mcp_tool_adapter
        .as_ref()
        .ok_or_else(|| AppError::Validation("MCP is not enabled".into()))?;

    let cfg = state.mcp_config_repo
        .get_by_id(config_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("MCP config {config_id} not found")))?;

    let parsed = db_config_to_parsed(&cfg);
    let connected = mcp_adapter.add_and_connect_server(parsed).await?;

    if connected {
        tracing::info!(config_id = %config_id, name = %cfg.server_name, "mcp_config.connected");
        Ok(Json(McpConfigStatusResponse {
            id: config_id.to_string(),
            server_name: cfg.server_name,
            status: "connected".into(),
            message: "MCP server connected successfully".into(),
        }))
    } else {
        let error = mcp_adapter
            .get_server_error(&cfg.server_name)
            .await
            .unwrap_or_else(|| "Unknown error".into());
        Err(AppError::Internal(format!(
            "Failed to connect to '{}': {error}",
            cfg.server_name
        )))
    }
}

/// POST /api/mcp-configs/:id/disconnect
pub async fn disconnect_server(
    State(state): State<Arc<AppState>>,
    Path(config_id): Path<Uuid>,
) -> Result<Json<McpConfigStatusResponse>, AppError> {
    let mcp_adapter = state
        .mcp_tool_adapter
        .as_ref()
        .ok_or_else(|| AppError::Validation("MCP is not enabled".into()))?;

    let cfg = state.mcp_config_repo
        .get_by_id(config_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("MCP config {config_id} not found")))?;

    mcp_adapter.disconnect_server_by_name(&cfg.server_name).await;

    tracing::info!(config_id = %config_id, name = %cfg.server_name, "mcp_config.disconnected");

    Ok(Json(McpConfigStatusResponse {
        id: config_id.to_string(),
        server_name: cfg.server_name,
        status: "disconnected".into(),
        message: "MCP server disconnected".into(),
    }))
}

/// DELETE /api/mcp-configs/:id
pub async fn delete_config(
    State(state): State<Arc<AppState>>,
    Path(config_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    state.mcp_config_repo.get_by_id(config_id).await?.ok_or_else(|| {
        AppError::NotFound(format!("MCP config {config_id} not found"))
    })?;

    if !state.mcp_config_repo.delete(config_id).await? {
        return Err(AppError::NotFound(format!(
            "MCP config {config_id} not found"
        )));
    }

    tracing::info!(config_id = %config_id, "mcp_config.deleted");

    Ok(StatusCode::NO_CONTENT)
}
