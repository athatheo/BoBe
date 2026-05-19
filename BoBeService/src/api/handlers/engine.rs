//! `/auth/status` and `/models`. Shared response shapes live here; per-engine
//! logic in `auth` / `models_cloud` / `models_local`.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Query, State};
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::models::engine_kind::EngineKind;

mod auth;
mod models_cloud;
mod models_local;

pub(crate) use auth::get_auth_status;

#[derive(Debug, Deserialize)]
pub(crate) struct ListModelsQuery {
    /// Preview override before flipping `Config.engine`; bad values 400 at decode.
    #[serde(default)]
    pub(crate) engine: Option<EngineKind>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ModelInfo {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) vision: bool,
    pub(crate) context_window: Option<i64>,
    /// `None` only for Ollama responses; `Some(1.0)` = base rate.
    pub(crate) multiplier: Option<f64>,
    pub(crate) default_reasoning_effort: Option<String>,
    pub(crate) supported_reasoning_efforts: Vec<String>,
    /// `"enabled"` / `"disabled"` / `"unconfigured"`.
    pub(crate) policy_state: Option<String>,
}

#[derive(Debug, Serialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ModelsSource {
    Upstream,
    /// Upstream call failed; `models` empty, `notice` carries the code, UI hides the picker.
    Fallback,
}

/// `list_models` always returns 200; failures emit `source: fallback` + this
/// notice. Swift maps `code` to action sets (Sign-in / Manage-plan / Use-local / Retry).
#[derive(Debug, Serialize)]
pub(crate) struct ModelsNotice {
    /// `AUTH_REQUIRED` / `NO_ENTITLEMENTS` / `SDK_ERROR` / `UPSTREAM_UNREACHABLE`.
    pub(crate) code: &'static str,
    pub(crate) message: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct ListModelsResponse {
    pub(crate) engine: EngineKind,
    pub(crate) models: Vec<ModelInfo>,
    pub(crate) source: ModelsSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) notice: Option<ModelsNotice>,
}

pub(crate) async fn list_models(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListModelsQuery>,
) -> Json<ListModelsResponse> {
    let cfg = state.config();
    let engine = q.engine.unwrap_or(cfg.engine.engine);

    let (models, source, notice) = match engine {
        EngineKind::Local => {
            let (models, notice) = models_local::list_local_models_resilient(
                &state.infra.http_client,
                cfg.engine.provider_base_url.as_deref(),
            )
            .await;
            let source = if notice.is_some() {
                ModelsSource::Fallback
            } else {
                ModelsSource::Upstream
            };
            (models, source, notice)
        }
        EngineKind::CopilotCloud => models_cloud::list_cloud_models_resilient(&state).await,
    };

    Json(ListModelsResponse {
        engine,
        models,
        source,
        notice,
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, reason = "tests panic on precondition failures")]
mod tests {
    use super::*;

    #[test]
    fn list_models_query_decodes_known_engine() {
        let q: ListModelsQuery =
            serde_json::from_str(r#"{"engine":"local"}"#).expect("decode local");
        assert_eq!(q.engine, Some(EngineKind::Local));
    }

    #[test]
    fn list_models_query_rejects_unknown_engine() {
        let result: Result<ListModelsQuery, _> =
            serde_json::from_str(r#"{"engine":"azure_openai"}"#);
        assert!(result.is_err(), "unknown engine should fail");
    }

    #[test]
    fn list_models_query_empty_is_none() {
        let q: ListModelsQuery = serde_json::from_str("{}").expect("decode empty");
        assert!(q.engine.is_none());
    }

    #[test]
    fn list_models_response_engine_serializes_to_wire_format() {
        let response = ListModelsResponse {
            engine: EngineKind::CopilotCloud,
            models: vec![],
            source: ModelsSource::Fallback,
            notice: None,
        };
        let json = serde_json::to_value(&response).expect("serialize");
        assert_eq!(json["engine"], serde_json::json!("copilot_cloud"));
        assert_eq!(json["source"], serde_json::json!("fallback"));
        assert!(
            json.get("notice").is_none(),
            "skip_serializing_if hides None"
        );
    }

    #[test]
    fn models_notice_serializes_with_static_code() {
        let notice = ModelsNotice {
            code: "AUTH_REQUIRED",
            message: "sign in".into(),
        };
        let json = serde_json::to_value(&notice).expect("serialize");
        assert_eq!(json["code"], serde_json::json!("AUTH_REQUIRED"));
        assert_eq!(json["message"], serde_json::json!("sign in"));
    }
}
