//! Helpers extracted from `registry.rs` — session-level transforms that don't
//! depend on `WorkerRegistry` state. Keeping them here lets `registry.rs` focus
//! on cache + lifecycle, while the per-class model/provider math and the
//! post-create `mode.set` RPC live next to one another.

use github_copilot_sdk::rpc::ModeSetRequest;
use github_copilot_sdk::session::Session;
use github_copilot_sdk::session_events::SessionMode;
use github_copilot_sdk::types::{ProviderConfig, SetModelOptions};

use crate::config::EngineConfig;
use crate::constants::DEFAULT_OLLAMA_V1_URL as DEFAULT_LOCAL_BASE_URL;
use crate::error::AppError;

use super::error::WorkerError;
use super::types::WorkerClass;

pub(super) fn session_extras_for_class(
    cfg: &EngineConfig,
    class: WorkerClass,
) -> (Option<String>, Option<ProviderConfig>, Option<String>) {
    let model = match class {
        WorkerClass::Chat => cfg.provider_chat_model.clone(),
        WorkerClass::Vision => cfg.provider_vision_model.clone(),
        WorkerClass::Goals | WorkerClass::Consolidate => cfg.provider_batch_model.clone(),
    };
    let reasoning = match class {
        WorkerClass::Chat => cfg.provider_chat_reasoning.clone(),
        WorkerClass::Vision => cfg.provider_vision_reasoning.clone(),
        WorkerClass::Goals | WorkerClass::Consolidate => cfg.provider_batch_reasoning.clone(),
    };

    let provider = if cfg.engine == crate::models::engine_kind::EngineKind::Local {
        let base_url = cfg
            .provider_base_url
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_LOCAL_BASE_URL.to_string());
        let mut p = ProviderConfig::default();
        p.provider_type = Some("openai".to_string());
        p.base_url = base_url;
        Some(p)
    } else {
        None
    };

    (model, provider, reasoning)
}

pub(super) fn log_shutdown(class: &str, result: Result<(), WorkerError>) {
    match result {
        Ok(()) => tracing::info!(class, "worker shut down"),
        Err(e) => tracing::warn!(class, err = %e, "worker shutdown failed"),
    }
}

/// Non-chat mode failures are fatal: autopilot drives batch auto-loop; without it `send_and_wait` hangs.
pub(super) async fn apply_runtime_mode(
    session: &Session,
    class: WorkerClass,
) -> Result<(), AppError> {
    let mode = match class.mode() {
        "interactive" => SessionMode::Interactive,
        "plan" => SessionMode::Plan,
        "autopilot" => SessionMode::Autopilot,
        other => {
            tracing::warn!(
                class = %class.name(),
                mode = other,
                "unknown session mode; leaving session at default"
            );
            return Ok(());
        }
    };
    match session.rpc().mode().set(ModeSetRequest { mode }).await {
        Ok(()) => Ok(()),
        Err(e) => {
            if class == WorkerClass::Chat {
                tracing::warn!(
                    class = %class.name(),
                    err = %e,
                    "chat session.mode.set failed; default mode is interactive — continuing"
                );
                Ok(())
            } else {
                Err(AppError::Internal(format!(
                    "session.mode.set({}) failed for {}: {e} — batch classes need autopilot",
                    class.mode(),
                    class.name()
                )))
            }
        }
    }
}

pub(super) async fn apply_reasoning_effort(
    session: &Session,
    class: WorkerClass,
    model: Option<&str>,
    reasoning_effort: Option<&str>,
) -> Result<(), AppError> {
    if class == WorkerClass::Chat {
        return Ok(());
    }
    let Some(reasoning_effort) = reasoning_effort else {
        return Ok(());
    };
    let Some(model) = model else {
        tracing::warn!(
            class = %class.name(),
            reasoning_effort,
            "reasoning override ignored because the SDK selected the model"
        );
        return Ok(());
    };
    if let Err(error) = session
        .set_model(
            model,
            Some(SetModelOptions::default().with_reasoning_effort(reasoning_effort)),
        )
        .await
    {
        tracing::warn!(
            class = %class.name(),
            model,
            reasoning_effort,
            err = %error,
            "reasoning override unsupported; continuing with model default"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_extras_select_per_class_reasoning() {
        let config = EngineConfig {
            provider_chat_reasoning: Some("high".into()),
            provider_batch_reasoning: Some("low".into()),
            provider_vision_reasoning: Some("medium".into()),
            ..EngineConfig::default()
        };

        assert_eq!(
            session_extras_for_class(&config, WorkerClass::Chat)
                .2
                .as_deref(),
            Some("high")
        );
        assert_eq!(
            session_extras_for_class(&config, WorkerClass::Goals)
                .2
                .as_deref(),
            Some("low")
        );
        assert_eq!(
            session_extras_for_class(&config, WorkerClass::Vision)
                .2
                .as_deref(),
            Some("medium")
        );
    }
}
