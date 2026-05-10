//! Engine + provider info endpoints.
//!
//! - `GET /auth/status` — wraps the SDK's `Client::get_auth_status()` so
//!   the wizard / Settings can show the user "signed in as @login"
//!   without shelling out to `copilot` or `gh`.
//! - `GET /models` — lists the models the user has access to in the
//!   current engine mode. Cloud mode hits the SDK's `Client::list_models()`
//!   (returns subscription-gated models). Local mode hits the user's
//!   Ollama at `:11434/api/tags` and reports installed model tags.
//!
//! Both endpoints call into the shared `WorkerRegistry`'s `ClientHandle`
//! via `ensure_started()` to lazy-spawn the Copilot CLI on first use.

use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::{Query, State};
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::error::AppError;

// ── /auth/status ────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub(crate) struct AuthStatusResponse {
    pub(crate) is_authenticated: bool,
    pub(crate) auth_type: Option<String>,
    pub(crate) host: Option<String>,
    pub(crate) login: Option<String>,
    pub(crate) status_message: Option<String>,
}

pub(crate) async fn get_auth_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<AuthStatusResponse>, AppError> {
    // We need a started Client to query auth — `Client::get_auth_status`
    // is an RPC into the spawned CLI. Lazy-spawn if cold.
    //
    // Note: this works in both engine modes. In `local` mode the bundled
    // CLI still has a notion of "is the user signed in to GitHub?" (used
    // by the cloud-fallback path); BoBe doesn't *need* GitHub auth in
    // local mode but the daemon still surfaces it accurately.
    let client = state
        .workers
        .client_handle()
        .ensure_started()
        .await
        .map_err(|e| AppError::Internal(format!("auth_status: client start failed: {e}")))?;

    let status = client
        .get_auth_status()
        .await
        .map_err(|e| AppError::Internal(format!("auth_status: get_auth_status failed: {e}")))?;

    Ok(Json(AuthStatusResponse {
        is_authenticated: status.is_authenticated,
        auth_type: status.auth_type,
        host: status.host,
        login: status.login,
        status_message: status.status_message,
    }))
}

// ── /models ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(crate) struct ListModelsQuery {
    /// Override the engine for this query — useful when the wizard wants
    /// to preview the local Ollama list before flipping `Config.engine`.
    /// When unset, falls back to the daemon's current `Config.engine`.
    #[serde(default)]
    pub(crate) engine: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ModelInfo {
    /// Stable identifier (cloud: `"claude-sonnet-4.5"`; local: an Ollama
    /// tag like `"qwen2.5:7b-instruct"`).
    pub(crate) id: String,
    /// Display name. Cloud SDK provides one; for Ollama we reuse the tag.
    pub(crate) name: String,
    /// Whether the model accepts images. Drives the Vision dropdown in
    /// Settings (which only shows vision-capable models).
    pub(crate) vision: bool,
    /// Maximum context window in tokens, when reported. Surfaced in
    /// Settings to help users gauge "is this model big enough for the
    /// 21K-token Copilot CLI system prompt?"
    pub(crate) context_window: Option<i64>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ListModelsResponse {
    /// `"copilot_cloud"` or `"local"` — echoes which engine was queried.
    pub(crate) engine: String,
    pub(crate) models: Vec<ModelInfo>,
}

pub(crate) async fn list_models(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListModelsQuery>,
) -> Result<Json<ListModelsResponse>, AppError> {
    let cfg = state.config();
    let engine = q.engine.as_deref().unwrap_or(cfg.engine.engine.as_str());

    match engine {
        "local" => list_local_models(cfg.engine.provider_base_url.as_deref())
            .await
            .map(|models| {
                Json(ListModelsResponse {
                    engine: "local".to_string(),
                    models,
                })
            }),
        _ => list_cloud_models(&state).await.map(|models| {
            Json(ListModelsResponse {
                engine: "copilot_cloud".to_string(),
                models,
            })
        }),
    }
}

async fn list_cloud_models(state: &Arc<AppState>) -> Result<Vec<ModelInfo>, AppError> {
    let client = state
        .workers
        .client_handle()
        .ensure_started()
        .await
        .map_err(|e| AppError::Internal(format!("list_models: client start failed: {e}")))?;

    let models = client
        .list_models()
        .await
        .map_err(|e| AppError::Internal(format!("list_models: SDK list_models failed: {e}")))?;

    Ok(models
        .into_iter()
        .map(|m| {
            let supports = m.capabilities.supports.as_ref();
            let limits = m.capabilities.limits.as_ref();
            ModelInfo {
                vision: supports.and_then(|s| s.vision).unwrap_or(false),
                context_window: limits.and_then(|l| l.max_context_window_tokens),
                id: m.id,
                name: m.name,
            }
        })
        .collect())
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    #[serde(default)]
    models: Vec<OllamaTag>,
}

#[derive(Debug, Deserialize)]
struct OllamaTag {
    name: String,
    #[serde(default)]
    details: Option<OllamaTagDetails>,
}

#[derive(Debug, Deserialize)]
struct OllamaTagDetails {
    /// Ollama reports the model family as a string like "qwen2", "llama".
    /// We use it as a weak hint for vision support — `qwen2_vl`, `llava`,
    /// etc. are common vision families.
    #[serde(default)]
    families: Vec<String>,
    #[serde(default)]
    family: Option<String>,
}

async fn list_local_models(base_url: Option<&str>) -> Result<Vec<ModelInfo>, AppError> {
    // `provider_base_url` looks like `http://127.0.0.1:11434/v1` for the
    // OpenAI-compat side. Ollama's native `/api/tags` is on the root,
    // not the `/v1` prefix.
    let root = base_url
        .map(|u| u.trim_end_matches('/').trim_end_matches("/v1").to_string())
        .unwrap_or_else(|| "http://127.0.0.1:11434".to_string());

    let url = format!("{root}/api/tags");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .map_err(|e| AppError::Internal(format!("list_local_models: client build: {e}")))?;

    let resp = client.get(&url).send().await.map_err(|e| {
        // Surface "not running" as a structured signal — the wizard /
        // Settings can render "Ollama not running" rather than a 500.
        AppError::ServiceUnavailable(format!("list_local_models: {e}"))
    })?;

    if !resp.status().is_success() {
        return Err(AppError::ServiceUnavailable(format!(
            "list_local_models: ollama returned {}",
            resp.status()
        )));
    }

    let parsed: OllamaTagsResponse = resp
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("list_local_models: parse: {e}")))?;

    Ok(parsed
        .models
        .into_iter()
        .map(|m| {
            let vision = guess_vision(m.details.as_ref());
            ModelInfo {
                id: m.name.clone(),
                name: m.name,
                vision,
                context_window: None, // Ollama doesn't report this in /api/tags
            }
        })
        .collect())
}

/// Heuristic: a model is vision-capable if its family name contains a
/// known vision marker. The Copilot CLI itself will reject vision calls
/// to non-vision models with a clear error, so this is a UX hint, not a
/// security/safety check.
fn guess_vision(details: Option<&OllamaTagDetails>) -> bool {
    let Some(d) = details else { return false };
    let mut all = d.families.clone();
    if let Some(f) = d.family.as_ref() {
        all.push(f.clone());
    }
    all.iter().any(|f| {
        let lower = f.to_lowercase();
        lower.contains("vl")
            || lower.contains("vision")
            || lower.contains("llava")
            || lower.contains("multimodal")
    })
}
