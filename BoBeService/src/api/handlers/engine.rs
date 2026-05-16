use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::{Query, State};
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::error::AppError;

#[derive(Debug, Serialize)]
pub(crate) struct AuthStatusResponse {
    pub(crate) is_authenticated: bool,
    pub(crate) auth_type: Option<String>,
    pub(crate) host: Option<String>,
    pub(crate) login: Option<String>,
    pub(crate) status_message: Option<String>,
    /// Bundled CLI path; Swift uses this to open Terminal for sign-in.
    pub(crate) cli_path: Option<String>,
    pub(crate) cli_version: Option<String>,
}

pub(crate) async fn get_auth_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<AuthStatusResponse>, AppError> {
    // `Client::start` extracts the bundled CLI as a side effect; required
    // before `embeddedcli::path()` returns Some.
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

    let cli_path =
        github_copilot_sdk::embeddedcli::path().map(|p| p.to_string_lossy().into_owned());
    let cli_version =
        github_copilot_sdk::embeddedcli::bundled_version().map(std::string::ToString::to_string);

    Ok(Json(AuthStatusResponse {
        is_authenticated: status.is_authenticated,
        auth_type: status.auth_type,
        host: status.host,
        login: status.login,
        status_message: status.status_message,
        cli_path,
        cli_version,
    }))
}

#[derive(Debug, Deserialize)]
pub(crate) struct ListModelsQuery {
    /// Override engine for preview before flipping `Config.engine`.
    #[serde(default)]
    pub(crate) engine: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ModelInfo {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) vision: bool,
    pub(crate) context_window: Option<i64>,
    /// `Some(1.0)` = base rate; `None` only for non-Copilot (Ollama) responses.
    pub(crate) multiplier: Option<f64>,
    pub(crate) default_reasoning_effort: Option<String>,
    pub(crate) supported_reasoning_efforts: Vec<String>,
    /// `"enabled"` / `"disabled"` / `"unconfigured"` — UI dims non-enabled entries.
    pub(crate) policy_state: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ListModelsResponse {
    pub(crate) engine: String,
    pub(crate) models: Vec<ModelInfo>,
}

pub(crate) async fn list_models(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListModelsQuery>,
) -> Result<Json<ListModelsResponse>, AppError> {
    let cfg = state.config();
    let engine = q.engine.as_deref().unwrap_or(cfg.engine.engine.as_str());

    use crate::constants::engine_kind::{COPILOT_CLOUD, LOCAL};
    match engine {
        LOCAL => list_local_models(cfg.engine.provider_base_url.as_deref())
            .await
            .map(|models| {
                Json(ListModelsResponse {
                    engine: LOCAL.to_string(),
                    models,
                })
            }),
        _ => list_cloud_models(&state).await.map(|models| {
            Json(ListModelsResponse {
                engine: COPILOT_CLOUD.to_string(),
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
                multiplier: m.billing.as_ref().map(|b| b.multiplier),
                default_reasoning_effort: m.default_reasoning_effort,
                supported_reasoning_efforts: m.supported_reasoning_efforts,
                policy_state: m.policy.as_ref().map(|p| p.state.clone()),
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
    #[serde(default)]
    families: Vec<String>,
    #[serde(default)]
    family: Option<String>,
}

async fn list_local_models(base_url: Option<&str>) -> Result<Vec<ModelInfo>, AppError> {
    // Ollama's `/api/tags` lives on the root, not the `/v1` OpenAI-compat prefix.
    let root = base_url.map_or_else(
        || crate::constants::DEFAULT_OLLAMA_BASE_URL.to_string(),
        |u| u.trim_end_matches('/').trim_end_matches("/v1").to_string(),
    );

    let url = format!("{root}/api/tags");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .map_err(|e| AppError::Internal(format!("list_local_models: client build: {e}")))?;

    let resp = client.get(&url).send().await.map_err(|e| {
        // Surface "not running" as 503 so UI can render "Ollama not running" instead of a 500.
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
                context_window: None,
                multiplier: None,
                default_reasoning_effort: None,
                supported_reasoning_efforts: Vec::new(),
                policy_state: None,
            }
        })
        .collect())
}

/// UX hint only; the Copilot CLI rejects vision calls to non-vision models.
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
