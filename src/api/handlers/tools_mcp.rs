use std::collections::HashMap;
use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::app_state::AppState;
use crate::error::AppError;
use crate::models::mcp_server_config::McpServerConfig;
use crate::tools::mcp::adapter::db_config_to_parsed;

#[derive(Debug, Serialize)]
pub struct McpServerResponse {
    pub id: String,
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub enabled: bool,
    pub connected: bool,
    pub tool_count: usize,
    pub excluded_tools: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct McpServerListResponse {
    pub servers: Vec<McpServerResponse>,
    pub count: usize,
    pub connected_count: usize,
}

#[derive(Debug, Deserialize)]
pub struct McpServerCreateRequest {
    #[serde(alias = "name")]
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

#[derive(Debug, Serialize)]
pub struct McpServerCreateResponse {
    pub name: String,
    pub connected: bool,
    pub tool_count: usize,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct McpServerDeleteResponse {
    pub name: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct McpServerReconnectResponse {
    pub name: String,
    pub connected: bool,
    pub tool_count: usize,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct McpServerUpdateRequest {
    #[serde(default)]
    pub excluded_tools: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct McpServerUpdateResponse {
    pub name: String,
    pub excluded_tools: Vec<String>,
    pub message: String,
}

/// GET /api/tools/mcp
pub async fn list_mcp_servers(
    State(state): State<Arc<AppState>>,
) -> Result<Json<McpServerListResponse>, AppError> {
    let configs = state.mcp_config_repo.get_all().await?;

    let mut servers = Vec::new();
    for cfg in &configs {
        let excluded_tools: Vec<String> = cfg
            .excluded_tools
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        let (connected, tool_count, runtime_error) =
            if let Some(ref adapter) = state.mcp_tool_adapter {
                match adapter.get_tools_for_server(&cfg.server_name).await {
                    Ok(tools) => (
                        true,
                        tools.len(),
                        adapter.get_server_error(&cfg.server_name).await,
                    ),
                    Err(e) => (
                        false,
                        0,
                        adapter
                            .get_server_error(&cfg.server_name)
                            .await
                            .or_else(|| Some(e.to_string())),
                    ),
                }
            } else {
                (false, 0, None)
            };

        servers.push(McpServerResponse {
            id: cfg.id.to_string(),
            name: cfg.server_name.clone(),
            command: cfg.command.clone(),
            args: cfg.args_vec(),
            env: cfg.env_map(),
            enabled: cfg.enabled,
            connected,
            tool_count,
            excluded_tools,
            error: runtime_error.or_else(|| cfg.last_error.clone()),
        });
    }

    let count = servers.len();
    let connected_count = servers.iter().filter(|s| s.connected).count();

    Ok(Json(McpServerListResponse {
        servers,
        count,
        connected_count,
    }))
}

/// POST /api/tools/mcp
pub async fn add_mcp_server(
    State(state): State<Arc<AppState>>,
    Json(body): Json<McpServerCreateRequest>,
) -> Result<Json<McpServerCreateResponse>, AppError> {
    if body.server_name.is_empty() {
        return Err(AppError::Validation("server_name must not be empty".into()));
    }
    if body.command.is_empty() {
        return Err(AppError::Validation("command must not be empty".into()));
    }

    let cfg = state.config.load();
    let blocked_cmds = cfg.mcp_blocked_commands_vec();
    let dangerous_keys = cfg.mcp_dangerous_env_keys_vec();
    crate::tools::mcp::security::validate_mcp_command(&body.command, blocked_cmds)?;
    crate::tools::mcp::security::validate_mcp_env(&body.env, dangerous_keys)?;

    let repo = state.mcp_config_repo.clone();

    if repo.get_by_name(&body.server_name).await?.is_some() {
        return Err(AppError::Validation(format!(
            "MCP server '{}' already exists",
            body.server_name
        )));
    }

    let args_json = serde_json::to_string(&body.args)?;
    let env_json = serde_json::to_string(&body.env)?;
    let excluded_json = serde_json::to_string(&body.excluded_tools)?;

    let mut cfg = McpServerConfig::new(body.server_name.clone(), body.command);
    cfg.args = args_json;
    cfg.env = env_json;
    cfg.enabled = body.enabled;
    cfg.timeout_seconds = body.timeout_seconds;
    cfg.excluded_tools = Some(excluded_json);

    let saved = repo.save(&cfg).await?;

    let mut connected = false;
    let mut tool_count = 0usize;
    let mut runtime_error: Option<String> = None;

    if saved.enabled
        && let Some(ref adapter) = state.mcp_tool_adapter
    {
        connected = adapter
            .add_and_connect_server(db_config_to_parsed(&saved))
            .await?;
        if connected {
            tool_count = adapter
                .get_tools_for_server(&saved.server_name)
                .await
                .map(|tools| tools.len())
                .unwrap_or(0);
        } else {
            runtime_error = adapter.get_server_error(&saved.server_name).await;
        }
    }

    tracing::info!(name = %saved.server_name, "tools.mcp_server_added");

    Ok(Json(McpServerCreateResponse {
        name: saved.server_name,
        connected,
        tool_count,
        message: if connected {
            "MCP server added and connected".into()
        } else {
            "MCP server added".into()
        },
        error: runtime_error,
    }))
}

/// DELETE /api/tools/mcp/:server_name
pub async fn remove_mcp_server(
    State(state): State<Arc<AppState>>,
    Path(server_name): Path<String>,
) -> Result<Json<McpServerDeleteResponse>, AppError> {
    let repo = state.mcp_config_repo.clone();

    let cfg = repo
        .get_by_name(&server_name)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("MCP server '{}' not found", server_name)))?;

    if let Some(ref adapter) = state.mcp_tool_adapter {
        let _ = adapter.disconnect_server_by_name(&cfg.server_name).await;
    }

    repo.delete(cfg.id).await?;

    tracing::info!(name = %server_name, "tools.mcp_server_removed");

    Ok(Json(McpServerDeleteResponse {
        name: server_name,
        message: "MCP server removed".into(),
    }))
}

/// PATCH /api/tools/mcp/:server_name
pub async fn update_mcp_server(
    State(state): State<Arc<AppState>>,
    Path(server_name): Path<String>,
    Json(body): Json<McpServerUpdateRequest>,
) -> Result<Json<McpServerUpdateResponse>, AppError> {
    let repo = state.mcp_config_repo.clone();
    let existing = repo
        .get_by_name(&server_name)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("MCP server '{}' not found", server_name)))?;

    let excluded_json = serde_json::to_string(&body.excluded_tools)?;
    let updated = repo
        .update(
            existing.id,
            None,
            None,
            None,
            None,
            None,
            Some(&excluded_json),
        )
        .await?
        .ok_or_else(|| AppError::NotFound(format!("MCP server '{}' not found", server_name)))?;

    if let Some(ref adapter) = state.mcp_tool_adapter {
        let was_connected = adapter.get_tools_for_server(&server_name).await.is_ok();
        if was_connected {
            let connected = adapter
                .add_and_connect_server(db_config_to_parsed(&updated))
                .await?;
            if !connected {
                warn!(name = %server_name, "tools.mcp_update_reconnect_failed");
            }
        }
    }

    tracing::info!(name = %server_name, "tools.mcp_server_updated");

    Ok(Json(McpServerUpdateResponse {
        name: updated.server_name,
        excluded_tools: body.excluded_tools,
        message: "MCP server updated".into(),
    }))
}

/// POST /api/tools/mcp/:server_name/reconnect
pub async fn reconnect_mcp_server(
    State(state): State<Arc<AppState>>,
    Path(server_name): Path<String>,
) -> Result<Json<McpServerReconnectResponse>, AppError> {
    let mcp_adapter = state
        .mcp_tool_adapter
        .as_ref()
        .ok_or_else(|| AppError::Validation("MCP is not enabled".into()))?;

    let repo = state.mcp_config_repo.clone();
    let _cfg = repo
        .get_by_name(&server_name)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("MCP server '{}' not found", server_name)))?;

    mcp_adapter.reconnect_server(&server_name).await?;
    let connected = mcp_adapter.get_tools_for_server(&server_name).await.is_ok();
    let tool_count = if connected {
        mcp_adapter
            .get_tools_for_server(&server_name)
            .await
            .map(|tools| tools.len())
            .unwrap_or(0)
    } else {
        0
    };
    let runtime_error = mcp_adapter.get_server_error(&server_name).await;

    tracing::info!(name = %server_name, "tools.mcp_reconnected");

    Ok(Json(McpServerReconnectResponse {
        name: server_name,
        connected,
        tool_count,
        message: if connected {
            "MCP server reconnected".into()
        } else {
            "MCP server reconnect failed".into()
        },
        error: runtime_error,
    }))
}
