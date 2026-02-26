use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::error::AppError;

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

#[derive(Debug, Deserialize)]
pub struct ToolUpdateRequest {
    pub enabled: bool,
}

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

/// PATCH /api/tools/:tool_name
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
