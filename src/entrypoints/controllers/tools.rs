use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::app_state::AppState;
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

// ── Handler ─────────────────────────────────────────────────────────────────

/// GET /api/tools
///
/// Lists all available tools from native providers and MCP servers.
/// Full tool registry integration will be wired later.
pub async fn list_tools(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ToolListResponse>, AppError> {
    let cfg = state.config();

    // Return built-in tool definitions (static list)
    let mut tools = Vec::new();
    let mut providers = vec!["bobe".to_owned()];

    if cfg.tools_enabled {
        let native_tools = [
            ("search_memories", "Search memories by semantic similarity", "memory"),
            ("search_context", "Search recent observations/context", "memory"),
            ("search_goal", "Search goals by semantic similarity", "goals"),
            ("get_goals", "Get all active goals", "goals"),
            ("get_souls", "Get active personality documents", "personality"),
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
            ("discover_installed_tools", "Discover installed dev tools", "code"),
            ("launch_coding_agent", "Launch an autonomous coding agent", "agents"),
            ("check_coding_agent", "Check status of a coding agent", "agents"),
            ("cancel_coding_agent", "Cancel a running coding agent", "agents"),
            ("list_coding_agents", "List all coding agents", "agents"),
        ];

        for (name, desc, category) in native_tools {
            tools.push(ToolResponse {
                name: name.into(),
                description: desc.into(),
                provider: "bobe".into(),
                enabled: true,
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
