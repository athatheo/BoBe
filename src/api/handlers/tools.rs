use std::collections::HashMap;
use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::models::mcp_server_config::McpServerConfig;
use crate::error::AppError;

// ── Schemas ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ToolResponse {
    pub name: String,
    pub description: String,
    pub provider: String,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ToolListResponse {
    pub tools: Vec<ToolResponse>,
    pub count: usize,
    pub providers: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ToolUpdateResponse {
    pub name: String,
    pub enabled: bool,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct McpServerResponse {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub enabled: bool,
    pub connected: bool,
    pub tool_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct McpServerListResponse {
    pub servers: Vec<McpServerResponse>,
    pub count: usize,
}

#[derive(Debug, Deserialize)]
pub struct McpServerCreateRequest {
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
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct McpServerDeleteResponse {
    pub name: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct McpServerReconnectResponse {
    pub name: String,
    pub status: String,
    pub message: String,
}

// ── Handlers ────────────────────────────────────────────────────────────────

/// GET /api/tools
pub async fn list_tools(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ToolListResponse>, AppError> {
    let cfg = state.config();

    let mut tools = Vec::new();
    let mut providers = vec!["bobe".to_owned()];

    if cfg.tools_enabled {
        let native_tools = [
            (
                "search_memories",
                "Search memories by semantic similarity",
                "memory",
            ),
            (
                "search_context",
                "Search recent observations/context",
                "memory",
            ),
            (
                "search_goal",
                "Search goals by semantic similarity",
                "goals",
            ),
            ("get_goals", "Get all active goals", "goals"),
            (
                "get_souls",
                "Get active personality documents",
                "personality",
            ),
            ("get_recent_context", "Get recent observations", "context"),
            ("create_memory", "Create a new memory", "memory"),
            ("update_memory", "Update an existing memory", "memory"),
            ("create_goal", "Create a new goal", "goals"),
            ("update_goal", "Update an existing goal", "goals"),
            ("complete_goal", "Mark a goal as completed", "goals"),
            ("archive_goal", "Archive a goal", "goals"),
            ("file_reader", "Read file contents", "filesystem"),
            ("list_directory", "List directory contents", "filesystem"),
            ("search_files", "Search for files by pattern", "filesystem"),
            ("fetch_url", "Fetch a URL and extract text", "web"),
            ("browser_history", "Search browser history", "web"),
            ("discover_git_repos", "Discover Git repositories", "code"),
            (
                "discover_installed_tools",
                "Discover installed dev tools",
                "code",
            ),
            (
                "launch_coding_agent",
                "Launch an autonomous coding agent",
                "agents",
            ),
            (
                "check_coding_agent",
                "Check status of a coding agent",
                "agents",
            ),
            (
                "cancel_coding_agent",
                "Cancel a running coding agent",
                "agents",
            ),
            ("list_coding_agents", "List all coding agents", "agents"),
        ];

        for (name, desc, category) in native_tools {
            let enabled = state
                .tool_registry
                .is_tool_enabled(name)
                .await
                .unwrap_or(true);

            tools.push(ToolResponse {
                name: name.into(),
                description: desc.into(),
                provider: "bobe".into(),
                enabled,
                category: Some(category.into()),
            });
        }
    }

    if cfg.mcp_enabled {
        providers.push("mcp".to_owned());
    }

    let count = tools.len();

    Ok(Json(ToolListResponse {
        tools,
        count,
        providers,
    }))
}

/// POST /api/tools/:tool_name/enable
pub async fn enable_tool(
    State(state): State<Arc<AppState>>,
    Path(tool_name): Path<String>,
) -> Result<Json<ToolUpdateResponse>, AppError> {
    // Refresh the index so the registry knows about all tools
    let _ = state.tool_registry.refresh_index().await;

    let success = state.tool_registry.enable_tool(&tool_name).await;

    if !success {
        return Err(AppError::NotFound(format!(
            "Tool '{}' not found",
            tool_name
        )));
    }

    tracing::info!(tool_name = %tool_name, "tools.enabled");

    Ok(Json(ToolUpdateResponse {
        name: tool_name.clone(),
        enabled: true,
        message: format!("Tool '{}' enabled", tool_name),
    }))
}

/// POST /api/tools/:tool_name/disable
pub async fn disable_tool(
    State(state): State<Arc<AppState>>,
    Path(tool_name): Path<String>,
) -> Result<Json<ToolUpdateResponse>, AppError> {
    let _ = state.tool_registry.refresh_index().await;

    let success = state.tool_registry.disable_tool(&tool_name).await;

    if !success {
        return Err(AppError::NotFound(format!(
            "Tool '{}' not found",
            tool_name
        )));
    }

    tracing::info!(tool_name = %tool_name, "tools.disabled");

    Ok(Json(ToolUpdateResponse {
        name: tool_name.clone(),
        enabled: false,
        message: format!("Tool '{}' disabled", tool_name),
    }))
}

/// PATCH /api/tools/:tool_name — update tool configuration (enable/disable).
pub async fn update_tool(
    State(state): State<Arc<AppState>>,
    Path(tool_name): Path<String>,
    Json(body): Json<ToolUpdateRequest>,
) -> Result<Json<ToolUpdateResponse>, AppError> {
    let _ = state.tool_registry.refresh_index().await;

    let success = if body.enabled {
        state.tool_registry.enable_tool(&tool_name).await
    } else {
        state.tool_registry.disable_tool(&tool_name).await
    };

    if !success {
        return Err(AppError::NotFound(format!(
            "Tool '{}' not found",
            tool_name
        )));
    }

    tracing::info!(tool_name = %tool_name, enabled = body.enabled, "tools.updated");

    Ok(Json(ToolUpdateResponse {
        name: tool_name.clone(),
        enabled: body.enabled,
        message: format!(
            "Tool '{}' {}",
            tool_name,
            if body.enabled { "enabled" } else { "disabled" }
        ),
    }))
}

#[derive(Debug, Deserialize)]
pub struct ToolUpdateRequest {
    pub enabled: bool,
}

/// GET /api/tools/mcp
pub async fn list_mcp_servers(
    State(state): State<Arc<AppState>>,
) -> Result<Json<McpServerListResponse>, AppError> {
    let configs = state.mcp_config_repo.get_all().await?;

    let mut servers = Vec::new();
    for cfg in &configs {
        // Get runtime tool count and error from adapter if available
        let (tool_count, runtime_error) = if let Some(ref adapter) = state.mcp_tool_adapter {
            let tools = adapter
                .get_tools_for_server(&cfg.server_name)
                .await
                .unwrap_or_default();
            let error = adapter.get_server_error(&cfg.server_name).await;
            (tools.len(), error)
        } else {
            (0, None)
        };

        servers.push(McpServerResponse {
            name: cfg.server_name.clone(),
            command: cfg.command.clone(),
            args: cfg.args_vec(),
            env: cfg.env_map(),
            enabled: cfg.enabled,
            connected: cfg.last_connected_at.is_some() && runtime_error.is_none(),
            tool_count,
            last_error: runtime_error.or_else(|| cfg.last_error.clone()),
        });
    }

    let count = servers.len();

    Ok(Json(McpServerListResponse { servers, count }))
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

    // Security validation (use configurable blocklists)
    let cfg = state.config.load();
    let blocked_cmds: Vec<String> = cfg.mcp_blocked_commands_vec();
    let dangerous_keys: Vec<String> = cfg.mcp_dangerous_env_keys_vec();
    crate::tools::mcp::security::validate_mcp_command(&body.command, &blocked_cmds)?;
    crate::tools::mcp::security::validate_mcp_env(&body.env, &dangerous_keys)?;

    let repo = state.mcp_config_repo.clone();

    if repo.get_by_name(&body.server_name).await?.is_some() {
        return Err(AppError::Validation(format!(
            "MCP server '{}' already exists",
            body.server_name
        )));
    }

    let args_json = serde_json::to_string(&body.args)?;
    let env_json = serde_json::to_string(&body.env)?;

    let mut cfg = McpServerConfig::new(body.server_name.clone(), body.command);
    cfg.args = args_json;
    cfg.env = env_json;
    cfg.enabled = body.enabled;
    cfg.timeout_seconds = body.timeout_seconds;

    let saved = repo.save(&cfg).await?;

    tracing::info!(name = %saved.server_name, "tools.mcp_server_added");

    Ok(Json(McpServerCreateResponse {
        name: saved.server_name,
        message: "MCP server added".into(),
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

    repo.delete(cfg.id).await?;

    tracing::info!(name = %server_name, "tools.mcp_server_removed");

    Ok(Json(McpServerDeleteResponse {
        name: server_name,
        message: "MCP server removed".into(),
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

    tracing::info!(name = %server_name, "tools.mcp_reconnected");

    Ok(Json(McpServerReconnectResponse {
        name: server_name,
        status: "ok".into(),
        message: "MCP server reconnected".into(),
    }))
}
