//! Cloud-path `/models` handler.
//!
//! Calls the JSON-RPC `models.list` method via the raw client (NOT
//! `client.list_models()`) and decodes into our own forgiving struct.
//!
//! We avoid the typed `Client::list_models()` because SDK 0.1.0's
//! `ModelBilling.multiplier` field is non-optional `f64`, while the
//! Copilot CLI sometimes returns `billing` objects without
//! `multiplier`. That serde mismatch shows up as `missing field
//! \`multiplier\`` and would otherwise mask a perfectly good model
//! list. Deserializing into our own shape — every field optional —
//! keeps us robust to upstream schema drift in either direction.
//!
//! On genuine call failure (auth, network, CLI crash), returns
//! `(empty_models, Fallback, Some(notice))`. The UI then renders the
//! notice as an error and hides the picker — there's no point
//! selecting a model the runtime won't be able to reach.

use std::sync::Arc;

use serde::Deserialize;

use crate::app_state::AppState;

use super::{ModelInfo, ModelsNotice, ModelsSource};

/// Tag identifying which step of the cloud fetch failed; used by
/// `list_cloud_models_resilient` to route the error to the right notice.
enum CloudFetchError {
    ClientStart(String),
    ModelsList(String),
    Decode(String),
    NoEntitlements,
}

pub(super) async fn list_cloud_models_resilient(
    state: &Arc<AppState>,
) -> (Vec<ModelInfo>, ModelsSource, Option<ModelsNotice>) {
    match list_cloud_models(state).await {
        Ok(models) => (models, ModelsSource::Upstream, None),
        Err(err) => {
            let notice = match err {
                CloudFetchError::ClientStart(msg) => classify_cloud_notice("client_start", &msg),
                CloudFetchError::ModelsList(msg) => classify_cloud_notice("models.list", &msg),
                CloudFetchError::Decode(msg) => ModelsNotice {
                    code: "SDK_ERROR",
                    message: format!("models.list: decode failed: {msg}"),
                },
                CloudFetchError::NoEntitlements => ModelsNotice {
                    code: "NO_ENTITLEMENTS",
                    message: "GitHub Copilot returned no models for this account.".to_string(),
                },
            };
            (Vec::new(), ModelsSource::Fallback, Some(notice))
        }
    }
}

async fn list_cloud_models(state: &Arc<AppState>) -> Result<Vec<ModelInfo>, CloudFetchError> {
    let client = state
        .runtime
        .workers
        .client_handle()
        .ensure_started()
        .await
        .map_err(|e| CloudFetchError::ClientStart(e.to_string()))?;

    let raw = client
        .call("models.list", None)
        .await
        .map_err(|e| CloudFetchError::ModelsList(e.to_string()))?;

    let parsed: RawListModelsResponse =
        serde_json::from_value(raw).map_err(|e| CloudFetchError::Decode(e.to_string()))?;

    if parsed.models.is_empty() {
        return Err(CloudFetchError::NoEntitlements);
    }

    Ok(parsed
        .models
        .into_iter()
        .filter_map(|m| {
            // Drop entries that lack the bare identifiers — those are the
            // only fields the UI actually needs. Everything else degrades
            // gracefully to a sensible default.
            let id = m.id?;
            let name = m.name.unwrap_or_else(|| id.clone());
            let cap = m.capabilities.as_ref();
            let supports = cap.and_then(|c| c.supports.as_ref());
            let limits = cap.and_then(|c| c.limits.as_ref());
            Some(ModelInfo {
                id,
                name,
                vision: supports.and_then(|s| s.vision).unwrap_or(false),
                context_window: limits.and_then(|l| l.max_context_window_tokens),
                multiplier: m.billing.and_then(|b| b.multiplier),
                default_reasoning_effort: m.default_reasoning_effort,
                supported_reasoning_efforts: m.supported_reasoning_efforts.unwrap_or_default(),
                policy_state: m.policy.and_then(|p| p.state),
            })
        })
        .collect())
}

/// Forgiving local mirror of the JSON-RPC `models.list` response. Every
/// field is optional so schema drift between SDK and CLI never breaks us.
#[derive(Debug, Deserialize)]
struct RawListModelsResponse {
    #[serde(default)]
    models: Vec<RawModel>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawModel {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    billing: Option<RawBilling>,
    #[serde(default)]
    capabilities: Option<RawCapabilities>,
    #[serde(default)]
    default_reasoning_effort: Option<String>,
    #[serde(default)]
    supported_reasoning_efforts: Option<Vec<String>>,
    #[serde(default)]
    policy: Option<RawPolicy>,
}

#[derive(Debug, Deserialize)]
struct RawBilling {
    #[serde(default)]
    multiplier: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct RawCapabilities {
    #[serde(default)]
    supports: Option<RawSupports>,
    #[serde(default)]
    limits: Option<RawLimits>,
}

#[derive(Debug, Deserialize)]
struct RawSupports {
    #[serde(default)]
    vision: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawLimits {
    #[serde(default)]
    max_context_window_tokens: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct RawPolicy {
    #[serde(default)]
    state: Option<String>,
}

/// Classify an opaque SDK error string into a `ModelsNotice` so the
/// UI can render a non-blocking banner with the right action set.
fn classify_cloud_notice(op: &str, err: &str) -> ModelsNotice {
    tracing::warn!(op = op, error = err, "copilot cloud call failed");

    let lower = err.to_lowercase();
    let mentions = |needle: &str| lower.contains(needle);

    if has_status_code(&lower, "401")
        || mentions("unauthorized")
        || mentions("token expired")
        || mentions("invalid token")
        || mentions("not authenticated")
        || mentions("no token")
        || mentions("sign in")
        || mentions("sign-in")
        || mentions("not logged in")
    {
        return ModelsNotice {
            code: "AUTH_REQUIRED",
            message: format!("{op}: GitHub Copilot session needs sign-in"),
        };
    }

    if has_status_code(&lower, "403")
        || mentions("forbidden")
        || mentions("not entitled")
        || mentions("no copilot")
        || mentions("entitlement")
    {
        return ModelsNotice {
            code: "NO_ENTITLEMENTS",
            message: format!("{op}: GitHub Copilot plan doesn't include this access"),
        };
    }

    ModelsNotice {
        code: "SDK_ERROR",
        message: format!("{op}: {err}"),
    }
}

/// Detects an HTTP status code substring inside an opaque error message.
/// Requires that the match is delimited by non-digit characters (or start /
/// end of string) so accidental matches inside port numbers, timeouts, etc.
/// don't fire.
fn has_status_code(haystack: &str, code: &str) -> bool {
    let mut start = 0;
    while let Some(rel) = haystack[start..].find(code) {
        let abs = start + rel;
        let prev_ok = abs == 0
            || !haystack[..abs]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_digit());
        let after = abs + code.len();
        let next_ok = after == haystack.len()
            || !haystack[after..]
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_digit());
        if prev_ok && next_ok {
            return true;
        }
        start = abs + code.len();
    }
    false
}
