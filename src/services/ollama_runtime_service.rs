use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::watch;

use crate::app_state::AppState;
use crate::binary_manager::DownloadProgress;
use crate::error::AppError;

/// Coordinates managed local Ollama readiness without owning persistence or UI flow.
pub(crate) struct OllamaRuntimeService {
    state: Arc<AppState>,
}

impl OllamaRuntimeService {
    pub(crate) fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    pub(crate) async fn prepare_managed_local_runtime(
        &self,
        progress_tx: &watch::Sender<DownloadProgress>,
    ) -> Result<PathBuf, AppError> {
        let binary_path = self
            .state
            .binary_manager
            .ensure_managed_ollama(progress_tx)
            .await?;
        self.state
            .binary_manager
            .validate_ollama_binary(&binary_path)
            .await?;
        self.state
            .ollama_manager
            .ensure_daemon_running(Some(binary_path.as_path()), true)
            .await?;
        Ok(binary_path)
    }

    pub(crate) async fn ensure_configured_runtime_ready(&self) -> Result<(), AppError> {
        let config = self.state.config();
        let auto_start = config.ollama.auto_start;
        let binary_path = self.resolve_runtime_binary_path(&config);
        drop(config);

        self.state
            .ollama_manager
            .ensure_daemon_running(binary_path.as_deref(), auto_start)
            .await
    }

    pub(crate) async fn ensure_model_ready(
        &self,
        model_name: &str,
        is_canceled: impl Fn() -> bool,
    ) -> Result<(), AppError> {
        let config = self.state.config();
        let auto_pull = config.ollama.auto_pull;
        drop(config);

        self.ensure_configured_runtime_ready().await?;

        if self.state.ollama_manager.has_model(model_name).await {
            tracing::info!(model = model_name, "ollama_runtime.model_ready");
            return Ok(());
        }

        if !auto_pull {
            return Err(AppError::LlmUnavailable(format!(
                "Model '{model_name}' not available and auto_pull is disabled"
            )));
        }

        self.state
            .ollama_manager
            .pull_model(model_name, is_canceled)
            .await
    }

    fn resolve_runtime_binary_path(&self, config: &crate::config::Config) -> Option<PathBuf> {
        resolve_binary_path_from_config(config.ollama.binary_path.as_deref(), || {
            self.state.binary_manager.find_managed_ollama()
        })
    }
}

/// Pure path-resolution logic: prefer an explicit config value, fall back to
/// the managed binary, return `None` if neither is available.
///
/// Extracted as a free function so the priority ordering is unit-testable
/// without needing a live AppState.
fn resolve_binary_path_from_config(
    config_path: Option<&str>,
    find_managed: impl FnOnce() -> Option<PathBuf>,
) -> Option<PathBuf> {
    config_path
        .filter(|p| !p.trim().is_empty())
        .map(PathBuf::from)
        .or_else(find_managed)
}

impl From<Arc<AppState>> for OllamaRuntimeService {
    fn from(state: Arc<AppState>) -> Self {
        Self::new(state)
    }
}

impl From<&Arc<AppState>> for OllamaRuntimeService {
    fn from(state: &Arc<AppState>) -> Self {
        Self::new(Arc::clone(state))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── resolve_binary_path_from_config ───────────────────────────────────────

    #[test]
    fn config_path_takes_priority_over_managed() {
        let result = resolve_binary_path_from_config(Some("/configured/ollama"), || {
            Some(PathBuf::from("/managed/ollama"))
        });
        assert_eq!(result, Some(PathBuf::from("/configured/ollama")));
    }

    #[test]
    fn falls_back_to_managed_when_config_path_is_none() {
        let result =
            resolve_binary_path_from_config(None, || Some(PathBuf::from("/managed/ollama")));
        assert_eq!(result, Some(PathBuf::from("/managed/ollama")));
    }

    #[test]
    fn falls_back_to_managed_when_config_path_is_empty() {
        let result =
            resolve_binary_path_from_config(Some(""), || Some(PathBuf::from("/managed/ollama")));
        assert_eq!(result, Some(PathBuf::from("/managed/ollama")));
    }

    #[test]
    fn falls_back_to_managed_when_config_path_is_whitespace() {
        let result =
            resolve_binary_path_from_config(Some("   "), || Some(PathBuf::from("/managed/ollama")));
        assert_eq!(result, Some(PathBuf::from("/managed/ollama")));
    }

    #[test]
    fn returns_none_when_both_config_and_managed_are_absent() {
        let result = resolve_binary_path_from_config(None, || None);
        assert_eq!(result, None);
    }

    #[test]
    fn managed_finder_is_not_called_when_config_path_is_set() {
        let mut called = false;
        resolve_binary_path_from_config(Some("/configured/ollama"), || {
            called = true;
            Some(PathBuf::from("/managed/ollama"))
        });
        assert!(
            !called,
            "managed finder should not be called when config path is set"
        );
    }
}
