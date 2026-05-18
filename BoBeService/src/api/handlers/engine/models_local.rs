//! Local-path `/models` handler — talks to Ollama via `/api/tags` on the
//! configured base URL.

use std::time::Duration;

use serde::Deserialize;

use crate::error::AppError;

use super::{ModelInfo, ModelsNotice};

/// Resilient wrapper around `list_local_models`. Never errors — when Ollama
/// is unreachable we return an empty list (no fallback catalog: local models
/// require a real install) paired with a `UPSTREAM_UNREACHABLE` notice so
/// the UI can show "start Ollama" guidance without a blocking error.
pub(super) async fn list_local_models_resilient(
    client: &reqwest::Client,
    base_url: Option<&str>,
) -> (Vec<ModelInfo>, Option<ModelsNotice>) {
    match list_local_models(client, base_url).await {
        Ok(models) => (models, None),
        Err(e) => {
            let msg = e.to_string();
            tracing::warn!(error = %msg, "list_local_models failed");
            (
                Vec::new(),
                Some(ModelsNotice {
                    code: "UPSTREAM_UNREACHABLE",
                    message: msg,
                }),
            )
        }
    }
}

async fn list_local_models(
    client: &reqwest::Client,
    base_url: Option<&str>,
) -> Result<Vec<ModelInfo>, AppError> {
    // Ollama's `/api/tags` lives on the root, not the `/v1` OpenAI-compat prefix.
    let root = base_url.map_or_else(
        || crate::constants::DEFAULT_OLLAMA_BASE_URL.to_string(),
        |u| u.trim_end_matches('/').trim_end_matches("/v1").to_string(),
    );

    let url = format!("{root}/api/tags");
    // 3s per-request timeout overrides the shared client's default for this
    // call only — Ollama not running needs to surface fast.
    let resp = client
        .get(&url)
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .map_err(|e| {
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
    let is_vision = |s: &str| {
        let lower = s.to_lowercase();
        lower.contains("vl")
            || lower.contains("vision")
            || lower.contains("llava")
            || lower.contains("multimodal")
    };
    d.families.iter().any(|f| is_vision(f)) || d.family.as_deref().is_some_and(is_vision)
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
