use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::app_state::AppState;
use crate::error::AppError;

// ── Schemas ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ModelInfo {
    pub name: String,
    pub size_bytes: i64,
    pub modified_at: String,
}

#[derive(Debug, Serialize)]
pub struct ModelsListResponse {
    pub backend: String,
    pub models: Vec<ModelInfo>,
    pub supports_pull: bool,
}

// ── Handler ─────────────────────────────────────────────────────────────────

/// GET /api/models
///
/// Lists available LLM models for the current backend. Only Ollama supports
/// model listing at the moment. Other backends return an empty list.
pub async fn list_models(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ModelsListResponse>, AppError> {
    let cfg = state.config();
    let backend = cfg.llm_backend.clone();

    let (models, supports_pull) = if backend == "ollama" {
        // Try to fetch from Ollama API
        let ollama_url = cfg.ollama_url.clone();
        match fetch_ollama_models(&ollama_url).await {
            Ok(m) => (m, true),
            Err(e) => {
                tracing::warn!(error = %e, "Failed to list Ollama models");
                (vec![], true)
            }
        }
    } else {
        (vec![], false)
    };

    Ok(Json(ModelsListResponse {
        backend,
        models,
        supports_pull,
    }))
}

async fn fetch_ollama_models(base_url: &str) -> Result<Vec<ModelInfo>, AppError> {
    let url = format!("{base_url}/api/tags");
    let resp = reqwest::get(&url).await?;

    if !resp.status().is_success() {
        return Err(AppError::LlmUnavailable(format!(
            "Ollama returned status {}",
            resp.status()
        )));
    }

    #[derive(serde::Deserialize)]
    struct OllamaTagsResponse {
        models: Option<Vec<OllamaModel>>,
    }

    #[derive(serde::Deserialize)]
    struct OllamaModel {
        name: String,
        #[serde(default)]
        size: i64,
        #[serde(default)]
        modified_at: String,
    }

    let body: OllamaTagsResponse = resp.json().await?;
    let models = body
        .models
        .unwrap_or_default()
        .into_iter()
        .map(|m| ModelInfo {
            name: m.name,
            size_bytes: m.size,
            modified_at: m.modified_at,
        })
        .collect();

    Ok(models)
}
