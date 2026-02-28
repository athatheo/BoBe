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
    let _ = state.tool_registry.refresh_index().await;
    let mut defs = state.tool_registry.get_all_tools(true).await;
    defs.sort_by(|a, b| a.name.cmp(&b.name));

    let mut tools = Vec::new();
    let mut providers: Vec<String> = Vec::new();
    for def in defs {
        let provider = state
            .tool_registry
            .get_source_for_tool(&def.name)
            .await
            .map(|s| s.name().to_owned())
            .unwrap_or_else(|| "unknown".to_owned());

        if provider == "bobe" && !cfg.tools.enabled {
            continue;
        }
        if provider == "mcp" && !cfg.mcp.enabled {
            continue;
        }

        let enabled = state
            .tool_registry
            .is_tool_enabled(&def.name)
            .await
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
