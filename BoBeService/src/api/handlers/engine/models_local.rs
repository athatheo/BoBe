//! Local-path `/models` handler — talks to Ollama via `/api/tags` on the
//! configured base URL.

use std::time::Duration;

use crate::error::AppError;
use crate::services::ollama::manager::{InstalledModelDetails, fetch_installed_models};

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
    let provider_url = base_url.unwrap_or(crate::constants::DEFAULT_OLLAMA_BASE_URL);
    let models = fetch_installed_models(client, provider_url, Duration::from_secs(3)).await?;

    Ok(models
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
fn guess_vision(details: Option<&InstalledModelDetails>) -> bool {
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
