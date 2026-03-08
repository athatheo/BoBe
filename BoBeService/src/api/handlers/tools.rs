use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::error::AppError;
use crate::tools::registry::ToolRegistry;

#[derive(Debug, Serialize)]
pub(crate) struct ToolResponse {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) provider: String,
    pub(crate) enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) category: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ToolListResponse {
    pub(crate) tools: Vec<ToolResponse>,
    pub(crate) count: usize,
    pub(crate) providers: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ToolUpdateResponse {
    pub(crate) name: String,
    pub(crate) enabled: bool,
    pub(crate) message: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ToolUpdateRequest {
    pub(crate) enabled: bool,
}

async fn set_tool_enabled(
    tool_registry: &Arc<ToolRegistry>,
    tool_name: &str,
    enabled: bool,
) -> Result<Json<ToolUpdateResponse>, AppError> {
    tool_registry.refresh_index().await?;
    let success = if enabled {
        tool_registry.enable_tool(tool_name)
    } else {
        tool_registry.disable_tool(tool_name)
    };

    if !success {
        return Err(AppError::NotFound(format!("Tool '{tool_name}' not found")));
    }

    tracing::info!(tool_name = %tool_name, enabled, "tools.updated");

    Ok(Json(ToolUpdateResponse {
        name: tool_name.to_owned(),
        enabled,
        message: format!(
            "Tool '{tool_name}' {}",
            if enabled { "enabled" } else { "disabled" }
        ),
    }))
}

pub(crate) async fn list_tools(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ToolListResponse>, AppError> {
    let cfg = state.config();
    state.tool_registry.refresh_index().await?;
    let mut defs = state.tool_registry.get_all_tools(true).await;
    defs.sort_by(|a, b| a.name.cmp(&b.name));

    let mut tools = Vec::new();
    let mut providers: Vec<String> = Vec::new();
    for def in defs {
        let provider = state
            .tool_registry
            .get_source_for_tool(&def.name)
            .await
            .map_or_else(|| "unknown".to_owned(), |s| s.name().to_owned());

        if provider == "bobe" && !cfg.tools.enabled {
            continue;
        }
        if provider == "mcp" && !cfg.mcp.enabled {
            continue;
        }

        let enabled = state
            .tool_registry
            .is_tool_enabled(&def.name)
            .unwrap_or(true);

        if !providers.contains(&provider) {
            providers.push(provider.clone());
        }

        tools.push(ToolResponse {
            name: def.name,
            description: def.description,
            provider,
            enabled,
            category: None,
        });
    }

    let count = tools.len();

    Ok(Json(ToolListResponse {
        tools,
        count,
        providers,
    }))
}

pub(crate) async fn enable_tool(
    State(state): State<Arc<AppState>>,
    Path(tool_name): Path<String>,
) -> Result<Json<ToolUpdateResponse>, AppError> {
    set_tool_enabled(&state.tool_registry, &tool_name, true).await
}

pub(crate) async fn disable_tool(
    State(state): State<Arc<AppState>>,
    Path(tool_name): Path<String>,
) -> Result<Json<ToolUpdateResponse>, AppError> {
    set_tool_enabled(&state.tool_registry, &tool_name, false).await
}

pub(crate) async fn update_tool(
    State(state): State<Arc<AppState>>,
    Path(tool_name): Path<String>,
    Json(body): Json<ToolUpdateRequest>,
) -> Result<Json<ToolUpdateResponse>, AppError> {
    set_tool_enabled(&state.tool_registry, &tool_name, body.enabled).await
}
