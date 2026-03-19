use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::config::LlmBackend;
use crate::error::AppError;
use crate::services::ollama_runtime_service::OllamaRuntimeService;

const MODEL_PULL_TIMEOUT: Duration = Duration::from_hours(2);

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
    /// Set when the Ollama daemon could not be reached; models will be empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) ollama_error: Option<String>,
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

    let (models, supports_pull, ollama_error) = if backend == LlmBackend::Ollama {
        let ollama_url = cfg.ollama.url.clone();
        drop(cfg);
        if let Err(e) = OllamaRuntimeService::from(&state)
            .ensure_configured_runtime_ready()
            .await
        {
            tracing::warn!(error = %e, "models.ollama_not_ready");
            return Ok(Json(ModelsListResponse {
                backend,
                models: vec![],
                supports_pull: true,
                ollama_error: Some(e.to_string()),
            }));
        }
        match fetch_ollama_models(&ollama_url).await {
            Ok(m) => (m, true, None),
            Err(e) => {
                tracing::warn!(error = %e, "Failed to list Ollama models");
                (vec![], true, Some(e.to_string()))
            }
        }
    } else {
        drop(cfg);
        (vec![], false, None)
    };

    Ok(Json(ModelsListResponse {
        backend,
        models,
        supports_pull,
        ollama_error,
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
    drop(cfg);
    let model_name = body.name.clone();
    let client = state.http_client.clone();
    let runtime_service = OllamaRuntimeService::from(&state);

    let stream = async_stream::stream! {
        if let Err(e) = runtime_service.ensure_configured_runtime_ready().await {
            yield Ok::<_, std::convert::Infallible>(Event::default().data(
                serde_json::json!({"status": "error", "detail": e.to_string()}).to_string()
            ));
            return;
        }

        crate::llm::ollama_manager::ensure_ollama_key_pair();

        let url = format!("{ollama_url}/api/pull");
        match client.post(&url)
            .json(&serde_json::json!({"name": model_name, "stream": true}))
            .timeout(MODEL_PULL_TIMEOUT)
            .send()
            .await
        {
            Ok(response) => {
                if !response.status().is_success() {
                    yield Ok(Event::default().data(
                        serde_json::json!({
                            "status": "error",
                            "detail": format!("Ollama pull returned {}", response.status())
                        }).to_string()
                    ));
                    return;
                }
                let mut byte_stream = response.bytes_stream();
                // Buffer across chunks so a JSON line split across TCP packets is
                // never silently dropped.
                let mut buf = String::new();
                'outer: while let Some(chunk) = byte_stream.next().await {
                    match chunk {
                        Ok(bytes) => {
                            for event in ndjson_pull_events(&mut buf, &bytes) {
                                yield Ok::<_, std::convert::Infallible>(Event::default().data(event.to_string()));
                            }
                        }
                        Err(e) => {
                            yield Ok(Event::default().data(
                                serde_json::json!({"status": "error", "detail": e.to_string()}).to_string()
                            ));
                            break 'outer;
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
    drop(cfg);

    OllamaRuntimeService::from(&state)
        .ensure_configured_runtime_ready()
        .await?;

    let url = format!("{}/api/delete", state.config().ollama.url);
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
        ollama_error: None,
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

/// Append `chunk` bytes to `buf`, drain all complete NDJSON lines, and return
/// one SSE event object per successfully parsed Ollama pull-progress line.
///
/// Keeping this as a free function (rather than inlining inside the stream)
/// makes it directly unit-testable and prevents the chunk-boundary regression
/// from silently creeping back in.
fn ndjson_pull_events(buf: &mut String, chunk: &[u8]) -> Vec<serde_json::Value> {
    buf.push_str(&String::from_utf8_lossy(chunk));
    let mut events = Vec::new();
    while let Some(newline_pos) = buf.find('\n') {
        let line = buf[..newline_pos].trim().to_string();
        buf.drain(..=newline_pos);
        if line.is_empty() {
            continue;
        }
        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&line) {
            let status = data
                .get("status")
                .and_then(|s| s.as_str())
                .unwrap_or("pulling");
            let mut event_data = serde_json::json!({"status": status});
            if let Some(total) = data.get("total").and_then(serde_json::Value::as_u64) {
                let completed = data
                    .get("completed")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0);
                let progress = if total > 0 {
                    (completed as f64 / total as f64 * 100.0) as u64
                } else {
                    0
                };
                event_data["progress"] = serde_json::json!(progress);
                event_data["completed"] = serde_json::json!(completed);
                event_data["total"] = serde_json::json!(total);
            }
            events.push(event_data);
        }
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── ndjson_pull_events ────────────────────────────────────────────────────

    #[test]
    fn ndjson_complete_line_in_single_chunk() {
        let mut buf = String::new();
        let events = ndjson_pull_events(&mut buf, b"{\"status\":\"pulling\"}\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["status"], "pulling");
        assert!(buf.is_empty(), "buf should be empty after a complete line");
    }

    #[test]
    fn ndjson_line_split_across_two_chunks() {
        let mut buf = String::new();
        // First half — no newline yet, should produce nothing
        let events = ndjson_pull_events(&mut buf, b"{\"status\":\"pulling\"");
        assert!(events.is_empty(), "incomplete line should yield no events");
        assert!(!buf.is_empty(), "fragment should be retained in buf");

        // Second half — completes the line
        let events = ndjson_pull_events(&mut buf, b"}\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["status"], "pulling");
        assert!(buf.is_empty());
    }

    #[test]
    fn ndjson_multiple_lines_in_one_chunk() {
        let mut buf = String::new();
        let chunk = b"{\"status\":\"pulling\"}\n{\"status\":\"verifying manifest\"}\n";
        let events = ndjson_pull_events(&mut buf, chunk);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["status"], "pulling");
        assert_eq!(events[1]["status"], "verifying manifest");
    }

    #[test]
    fn ndjson_progress_fields_computed_correctly() {
        let mut buf = String::new();
        let chunk = b"{\"status\":\"downloading\",\"completed\":500,\"total\":1000}\n";
        let events = ndjson_pull_events(&mut buf, chunk);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["progress"], 50u64);
        assert_eq!(events[0]["completed"], 500u64);
        assert_eq!(events[0]["total"], 1000u64);
    }

    #[test]
    fn ndjson_invalid_json_lines_silently_skipped() {
        let mut buf = String::new();
        let chunk = b"not-json-at-all\n{\"status\":\"success\"}\n";
        let events = ndjson_pull_events(&mut buf, chunk);
        assert_eq!(
            events.len(),
            1,
            "only the valid JSON line should produce an event"
        );
        assert_eq!(events[0]["status"], "success");
    }

    #[test]
    fn ndjson_empty_and_whitespace_lines_skipped() {
        let mut buf = String::new();
        let events = ndjson_pull_events(&mut buf, b"\n   \n{\"status\":\"pulling\"}\n");
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn ndjson_trailing_fragment_retained_in_buf() {
        let mut buf = String::new();
        let chunk = b"{\"status\":\"a\"}\n{\"status\":\"b\"}\n{\"status\":\"partial";
        let events = ndjson_pull_events(&mut buf, chunk);
        assert_eq!(events.len(), 2);
        assert_eq!(
            buf, "{\"status\":\"partial",
            "partial line should remain in buf"
        );
    }

    // ── ModelsListResponse serde ──────────────────────────────────────────────

    #[test]
    fn models_list_ollama_error_none_is_omitted_from_json() {
        let resp = ModelsListResponse {
            backend: LlmBackend::Ollama,
            models: vec![],
            supports_pull: true,
            ollama_error: None,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert!(
            json.get("ollama_error").is_none(),
            "ollama_error:None must be omitted so the Swift side never sees null"
        );
    }

    #[test]
    fn models_list_ollama_error_some_is_present_in_json() {
        let resp = ModelsListResponse {
            backend: LlmBackend::Ollama,
            models: vec![],
            supports_pull: true,
            ollama_error: Some("daemon not running".into()),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["ollama_error"], "daemon not running");
    }

    #[test]
    fn models_list_non_ollama_backend_omits_error_field() {
        let resp = ModelsListResponse {
            backend: LlmBackend::Openai,
            models: vec![],
            supports_pull: false,
            ollama_error: None,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert!(json.get("ollama_error").is_none());
        assert_eq!(json["backend"], "openai");
    }
}
