use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::config::LlmBackend;
use crate::error::AppError;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ModelInfo {
    pub(crate) name: String,
    pub(crate) size_bytes: i64,
    pub(crate) modified_at: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct ModelsListResponse {
    pub(crate) backend: LlmBackend,
    pub(crate) models: Vec<ModelInfo>,
    pub(crate) supports_pull: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PullModelRequest {
    pub(crate) name: String,
}

/// Only Ollama supports model listing; other backends return an empty list.
pub(crate) async fn list_models(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ModelsListResponse>, AppError> {
    let cfg = state.config();
    let backend = cfg.llm.backend;

    let (models, supports_pull) = if backend == LlmBackend::Ollama {
        let ollama_url = cfg.ollama.url.clone();
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

/// Streams Ollama model pull progress as SSE events.
pub(crate) async fn pull_model(
    State(state): State<Arc<AppState>>,
    Json(body): Json<PullModelRequest>,
) -> axum::response::Sse<
    std::pin::Pin<
        Box<
            dyn futures::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>
                + Send,
        >,
    >,
> {
    use axum::response::sse::Event;
    use tokio_stream::StreamExt;

    let cfg = state.config();
    if cfg.llm.backend != LlmBackend::Ollama {
        let stream = async_stream::stream! {
            yield Ok::<_, std::convert::Infallible>(
                Event::default()
                    .data(serde_json::json!({"status": "error", "detail": "Model pull only supported for Ollama backend"}).to_string())
            );
        };
        return axum::response::Sse::new(Box::pin(stream));
    }

    let ollama_url = cfg.ollama.url.clone();
    let model_name = body.name.clone();
    let client = state.http_client.clone();

    let stream = async_stream::stream! {
        let url = format!("{ollama_url}/api/pull");
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
                                    if let Some(total) = data.get("total").and_then(serde_json::Value::as_u64) {
                                        let completed = data.get("completed").and_then(serde_json::Value::as_u64).unwrap_or(0);
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

/// Ollama-only. Deletes a locally installed model.
pub(crate) async fn delete_model(
    State(state): State<Arc<AppState>>,
    Path(model_name): Path<String>,
) -> Result<StatusCode, AppError> {
    let cfg = state.config();
    if cfg.llm.backend != LlmBackend::Ollama {
        return Err(AppError::Validation(
            "Model deletion only supported for Ollama backend".into(),
        ));
    }

    let url = format!("{}/api/delete", cfg.ollama.url);
    let resp = state
        .http_client
        .delete(&url)
        .json(&serde_json::json!({"name": model_name}))
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await?;

    if resp.status().is_success() {
        tracing::info!(model = %model_name, "models.deleted");
        Ok(StatusCode::NO_CONTENT)
    } else {
        let status = resp.status();
        tracing::error!(model = %model_name, status = %status, "models.delete_failed");
        Err(AppError::Internal(format!(
            "Failed to delete: HTTP {status}"
        )))
    }
}

/// 1-hour TTL cache for registry models.
static REGISTRY_CACHE: std::sync::LazyLock<moka::future::Cache<(), Vec<ModelInfo>>> =
    std::sync::LazyLock::new(|| {
        moka::future::Cache::builder()
            .max_capacity(1)
            .time_to_live(std::time::Duration::from_hours(1))
            .build()
    });

/// Trending models from ollama.com (cached 1h).
pub(crate) async fn list_registry_models() -> Json<ModelsListResponse> {
    let models = REGISTRY_CACHE
        .try_get_with((), fetch_registry_models())
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "Failed to fetch registry models");
            Vec::new()
        });

    Json(ModelsListResponse {
        backend: LlmBackend::Ollama,
        models,
        supports_pull: true,
    })
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
