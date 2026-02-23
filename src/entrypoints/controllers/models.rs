use std::sync::{Arc, OnceLock};
use std::time::Instant;

use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::app_state::AppState;
use crate::error::AppError;

// ── Schemas ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Deserialize)]
pub struct PullModelRequest {
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct PullModelResponse {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct DeleteModelResponse {
    pub ok: bool,
    pub message: String,
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

/// POST /api/models/pull
///
/// Pull (download) a model by name. Returns SSE stream with progress events.
pub async fn pull_model(
    State(state): State<Arc<AppState>>,
    Json(body): Json<PullModelRequest>,
) -> axum::response::Sse<std::pin::Pin<Box<dyn futures::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>> + Send>>> {
    use axum::response::sse::Event;
    use tokio_stream::StreamExt;

    let cfg = state.config();
    if cfg.llm_backend != "ollama" {
        let stream = async_stream::stream! {
            yield Ok::<_, std::convert::Infallible>(
                Event::default()
                    .data(serde_json::json!({"status": "error", "detail": "Model pull only supported for Ollama backend"}).to_string())
            );
        };
        return axum::response::Sse::new(Box::pin(stream));
    }

    let ollama_url = cfg.ollama_url.clone();
    let model_name = body.name.clone();

    let stream = async_stream::stream! {
        let url = format!("{}/api/pull", ollama_url);
        let client = reqwest::Client::new();
        match client.post(&url)
            .json(&serde_json::json!({"name": model_name, "stream": true}))
            .send()
            .await
        {
            Ok(response) => {
                let mut byte_stream = response.bytes_stream();
                while let Some(chunk) = byte_stream.next().await {
                    match chunk {
                        Ok(bytes) => {
                            for line in String::from_utf8_lossy(&bytes).lines() {
                                let line = line.trim();
                                if line.is_empty() { continue; }
                                if let Ok(data) = serde_json::from_str::<serde_json::Value>(line) {
                                    let status = data.get("status").and_then(|s| s.as_str()).unwrap_or("pulling");
                                    let mut event_data = serde_json::json!({"status": status});
                                    if let Some(total) = data.get("total").and_then(|t| t.as_u64()) {
                                        let completed = data.get("completed").and_then(|c| c.as_u64()).unwrap_or(0);
                                        let progress = if total > 0 { (completed as f64 / total as f64 * 100.0) as u64 } else { 0 };
                                        event_data["progress"] = serde_json::json!(progress);
                                        event_data["completed"] = serde_json::json!(completed);
                                        event_data["total"] = serde_json::json!(total);
                                    }
                                    yield Ok::<_, std::convert::Infallible>(Event::default().data(event_data.to_string()));
                                }
                            }
                        }
                        Err(e) => {
                            yield Ok(Event::default().data(
                                serde_json::json!({"status": "error", "detail": e.to_string()}).to_string()
                            ));
                            break;
                        }
                    }
                }
                yield Ok(Event::default().data(serde_json::json!({"status": "complete"}).to_string()));
            }
            Err(e) => {
                yield Ok(Event::default().data(
                    serde_json::json!({"status": "error", "detail": e.to_string()}).to_string()
                ));
            }
        }
    };

    axum::response::Sse::new(Box::pin(stream))
}

/// DELETE /api/models/{model_name}
///
/// Delete a locally installed model via the Ollama API.
pub async fn delete_model(
    State(state): State<Arc<AppState>>,
    Path(model_name): Path<String>,
) -> Result<Json<DeleteModelResponse>, AppError> {
    let cfg = state.config();
    if cfg.llm_backend != "ollama" {
        return Ok(Json(DeleteModelResponse {
            ok: false,
            message: "Model deletion only supported for Ollama backend".into(),
        }));
    }

    let url = format!("{}/api/delete", cfg.ollama_url);
    let resp = reqwest::Client::new()
        .delete(&url)
        .json(&serde_json::json!({"name": model_name}))
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await?;

    if resp.status().is_success() {
        tracing::info!(model = %model_name, "models.deleted");
        Ok(Json(DeleteModelResponse {
            ok: true,
            message: format!("Model '{model_name}' deleted"),
        }))
    } else {
        let status = resp.status();
        tracing::error!(model = %model_name, status = %status, "models.delete_failed");
        Ok(Json(DeleteModelResponse {
            ok: false,
            message: format!("Failed to delete: HTTP {status}"),
        }))
    }
}

// Cache for registry models (1 hour TTL)
static REGISTRY_CACHE: OnceLock<Mutex<(Option<Vec<ModelInfo>>, Option<Instant>)>> = OnceLock::new();
const REGISTRY_CACHE_TTL_SECS: u64 = 3600;

/// GET /api/models/registry
///
/// List trending models from the Ollama public registry (ollama.com).
/// Cached for 1 hour.
pub async fn list_registry_models() -> Json<ModelsListResponse> {
    let cache = REGISTRY_CACHE.get_or_init(|| Mutex::new((None, None)));
    let mut guard = cache.lock().await;

    // Return cached if fresh
    if let (Some(ref models), Some(ref ts)) = *guard
        && ts.elapsed().as_secs() < REGISTRY_CACHE_TTL_SECS {
            return Json(ModelsListResponse {
                backend: "ollama".into(),
                models: models.clone(),
                supports_pull: true,
            });
        }

    // Fetch from ollama.com
    match fetch_registry_models().await {
        Ok(models) => {
            *guard = (Some(models.clone()), Some(Instant::now()));
            Json(ModelsListResponse {
                backend: "ollama".into(),
                models,
                supports_pull: true,
            })
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to fetch registry models");
            // Return stale cache or empty
            let models = guard.0.clone().unwrap_or_default();
            Json(ModelsListResponse {
                backend: "ollama".into(),
                models,
                supports_pull: true,
            })
        }
    }
}

async fn fetch_registry_models() -> Result<Vec<ModelInfo>, AppError> {
    let resp = reqwest::Client::new()
        .get("https://ollama.com/api/tags")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(AppError::Internal("Ollama registry returned error".into()));
    }

    #[derive(serde::Deserialize)]
    struct RegistryResponse {
        models: Option<Vec<RegistryModel>>,
    }

    #[derive(serde::Deserialize)]
    struct RegistryModel {
        #[serde(default)]
        name: String,
        #[serde(default)]
        size: i64,
        #[serde(default)]
        modified_at: String,
    }

    let body: RegistryResponse = resp.json().await?;
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
