//! Ollama daemon lifecycle: detect running, start managed binary,
//! pull models with streamed progress, list installed tags.
//!
//! This module is the daemon-side counterpart to `binary_manager` —
//! `binary_manager` ensures the binary file exists; `ollama_manager`
//! drives the running process. The `services::ollama_install_service`
//! orchestrator strings them together for the wizard install flow.
//!
//! Detection-first policy: if the user already runs Ollama (Homebrew,
//! official installer, prior BoBe install), `health_check()` returns
//! true and we reuse theirs. We only start a managed `ollama serve`
//! when the user has nothing on `:11434`. Models the user has already
//! pulled in `~/.ollama/models` are visible to us regardless of who
//! started the daemon.

use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use serde::Deserialize;
use tokio::sync::{Mutex, watch};
use tracing::{info, warn};

use crate::error::AppError;

const HEALTH_TIMEOUT: Duration = Duration::from_secs(5);
const PULL_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);
const STARTUP_POLL_INTERVAL: Duration = Duration::from_secs(1);
const STARTUP_MAX_ATTEMPTS: u32 = 30;

/// Streamed model-pull progress. `percent` is computed from
/// `completed / total` reported by Ollama's NDJSON protocol; both
/// fields can be `None` early in the pull (manifest fetch phase).
#[derive(Debug, Clone, Default)]
pub(crate) struct PullProgress {
    pub(crate) status: String,
    pub(crate) completed_bytes: Option<u64>,
    pub(crate) total_bytes: Option<u64>,
    pub(crate) percent: Option<u8>,
}

#[derive(Debug, Deserialize)]
struct TagsResponse {
    #[serde(default)]
    models: Vec<TagInfo>,
}

#[derive(Debug, Deserialize)]
struct TagInfo {
    name: String,
}

#[derive(Debug, Deserialize)]
struct OllamaPullEvent {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    completed: Option<u64>,
    #[serde(default)]
    total: Option<u64>,
    #[serde(default)]
    error: Option<String>,
}

/// Manages an Ollama daemon: detection, optional spawn, model pulls.
///
/// Stateful only with respect to the (optional) child process we
/// spawned. If `ensure_daemon_running` finds an externally-managed
/// daemon, `child` stays `None` and `stop()` is a no-op.
pub(crate) struct OllamaManager {
    http_client: Arc<reqwest::Client>,
    base_url: String,
    /// Owned child if WE started Ollama; `None` if reusing user's daemon.
    child: Mutex<Option<tokio::process::Child>>,
}

impl OllamaManager {
    /// `base_url` should be the host root, e.g. `http://127.0.0.1:11434`
    /// (no `/v1` suffix — Ollama's native API isn't OpenAI-compat-prefixed).
    pub(crate) fn new(http_client: Arc<reqwest::Client>, base_url: &str) -> Self {
        Self {
            http_client,
            base_url: base_url.trim_end_matches('/').to_string(),
            child: Mutex::new(None),
        }
    }

    /// Cheap GET on `/api/tags`; success means "Ollama is reachable on
    /// this base URL." Used as both health check and "did the daemon
    /// finish starting up?" probe.
    pub(crate) async fn health_check(&self) -> bool {
        let url = format!("{}/api/tags", self.base_url);
        match self
            .http_client
            .get(&url)
            .timeout(HEALTH_TIMEOUT)
            .send()
            .await
        {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    /// Make sure an Ollama daemon is responding on `base_url`. If one
    /// already is, returns immediately. If none and `auto_start` is
    /// `true`, spawns `binary_path serve`, polls health for up to 30s,
    /// retains the `Child` for `stop()` cleanup. If `auto_start` is
    /// `false` and no daemon exists, returns
    /// `AppError::ServiceUnavailable`.
    pub(crate) async fn ensure_daemon_running(
        &self,
        binary_path: Option<&Path>,
        auto_start: bool,
    ) -> Result<(), AppError> {
        if self.health_check().await {
            info!("ollama_manager.already_running");
            return Ok(());
        }

        if !auto_start {
            return Err(AppError::ServiceUnavailable(
                "Ollama not running and auto_start=false".into(),
            ));
        }

        let Some(binary) = binary_path else {
            return Err(AppError::Config(
                "Ollama binary path required when auto_start=true and no daemon is running".into(),
            ));
        };

        self.spawn_daemon(binary).await
    }

    async fn spawn_daemon(&self, binary: &Path) -> Result<(), AppError> {
        info!(binary = %binary.display(), "ollama_manager.spawning");

        // OLLAMA_HOST + OLLAMA_ORIGINS pin the daemon to localhost so it
        // can't be reached from other machines. Matches the security
        // posture of the BoBe daemon itself.
        let child = tokio::process::Command::new(binary)
            .arg("serve")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .env("OLLAMA_HOST", "127.0.0.1:11434")
            .env("OLLAMA_ORIGINS", "http://127.0.0.1:*")
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| AppError::Internal(format!("ollama spawn: {e}")))?;

        *self.child.lock().await = Some(child);

        for attempt in 0..STARTUP_MAX_ATTEMPTS {
            tokio::time::sleep(STARTUP_POLL_INTERVAL).await;
            if self.health_check().await {
                info!(attempts = attempt + 1, "ollama_manager.started");
                return Ok(());
            }
        }

        // Startup timed out — best-effort kill the child so we don't
        // leak a half-running process.
        if let Some(mut child) = self.child.lock().await.take()
            && let Err(e) = child.start_kill()
        {
            warn!(err = %e, "ollama_manager.kill_after_timeout_failed");
        }

        Err(AppError::ServiceUnavailable(format!(
            "Ollama failed to become healthy within {}s",
            STARTUP_MAX_ATTEMPTS
        )))
    }

    /// Pull a model from `registry.ollama.ai` via `/api/pull`. Streams
    /// NDJSON progress events to `progress_tx`. Idempotent — pulling
    /// an already-installed model just resolves immediately.
    ///
    /// `is_canceled` is checked between chunks so the wizard's cancel
    /// button can abort a multi-GB download promptly.
    pub(crate) async fn pull_model(
        &self,
        name: &str,
        progress_tx: &watch::Sender<PullProgress>,
        is_canceled: impl Fn() -> bool,
    ) -> Result<(), AppError> {
        let url = format!("{}/api/pull", self.base_url);
        info!(model = name, "ollama_manager.pull_starting");

        let resp = self
            .http_client
            .post(&url)
            .json(&serde_json::json!({"name": name}))
            .timeout(PULL_TIMEOUT)
            .send()
            .await
            .map_err(|e| AppError::ServiceUnavailable(format!("ollama pull: {e}")))?;

        if !resp.status().is_success() {
            return Err(AppError::ServiceUnavailable(format!(
                "ollama pull returned {}",
                resp.status()
            )));
        }

        let mut stream = resp.bytes_stream();
        let mut buf = String::new();

        while let Some(chunk) = stream.next().await {
            if is_canceled() {
                info!(model = name, "ollama_manager.pull_canceled");
                return Err(AppError::Conflict("Model pull canceled".into()));
            }

            let bytes = chunk
                .map_err(|e| AppError::ServiceUnavailable(format!("pull stream: {e}")))?;
            buf.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(nl) = buf.find('\n') {
                let line = buf[..nl].trim().to_string();
                buf.drain(..=nl);
                if line.is_empty() {
                    continue;
                }
                let Ok(event) = serde_json::from_str::<OllamaPullEvent>(&line) else {
                    continue;
                };

                if let Some(err) = event.error {
                    return Err(AppError::ServiceUnavailable(format!("ollama pull: {err}")));
                }

                let status = event.status.clone().unwrap_or_default();
                let percent = match (event.completed, event.total) {
                    (Some(c), Some(t)) if t > 0 => {
                        Some(((c as f64 / t as f64) * 100.0).min(100.0) as u8)
                    }
                    _ => None,
                };

                if status == "success" {
                    progress_tx
                        .send(PullProgress {
                            status: "success".into(),
                            completed_bytes: event.completed,
                            total_bytes: event.total,
                            percent: Some(100),
                        })
                        .ok();
                    info!(model = name, "ollama_manager.pull_complete");
                    return Ok(());
                }

                progress_tx
                    .send(PullProgress {
                        status,
                        completed_bytes: event.completed,
                        total_bytes: event.total,
                        percent,
                    })
                    .ok();
            }
        }

        // Stream ended without a `"status":"success"` line — typically
        // means the connection dropped mid-pull. Caller should retry.
        Err(AppError::ServiceUnavailable(
            "Ollama pull ended without success confirmation".into(),
        ))
    }

    /// Returns the list of installed model tags via `/api/tags`. Empty
    /// list on parse failure (logged).
    pub(crate) async fn list_installed_models(&self) -> Result<Vec<String>, AppError> {
        let url = format!("{}/api/tags", self.base_url);
        let resp = self
            .http_client
            .get(&url)
            .timeout(HEALTH_TIMEOUT)
            .send()
            .await
            .map_err(|e| AppError::ServiceUnavailable(format!("ollama tags: {e}")))?;

        if !resp.status().is_success() {
            return Err(AppError::ServiceUnavailable(format!(
                "ollama tags returned {}",
                resp.status()
            )));
        }

        let tags: TagsResponse = resp
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("ollama tags parse: {e}")))?;
        Ok(tags.models.into_iter().map(|m| m.name).collect())
    }

    /// Best-effort SIGTERM of our managed daemon. No-op if we never
    /// spawned one (i.e. user's existing Ollama is in use). Held for
    /// future wiring into the daemon-shutdown path; not called today.
    #[allow(dead_code)]
    pub(crate) async fn stop(&self) {
        let mut guard = self.child.lock().await;
        let Some(mut child) = guard.take() else {
            return;
        };
        if let Err(e) = child.start_kill() {
            warn!(err = %e, "ollama_manager.stop_kill_failed");
            return;
        }
        match tokio::time::timeout(Duration::from_secs(5), child.wait()).await {
            Ok(Ok(status)) => info!(status = %status, "ollama_manager.stopped"),
            Ok(Err(e)) => warn!(err = %e, "ollama_manager.stop_wait_failed"),
            Err(_) => warn!("ollama_manager.stop_timeout"),
        }
    }

    /// Convenience helper: parse the user's `provider_base_url`
    /// (`http://host/v1`) into the Ollama-native root (`http://host`).
    pub(crate) fn root_from_provider_url(provider_url: &str) -> String {
        provider_url
            .trim_end_matches('/')
            .trim_end_matches("/v1")
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::OllamaManager;

    #[test]
    fn root_from_provider_url_strips_v1() {
        assert_eq!(
            OllamaManager::root_from_provider_url("http://127.0.0.1:11434/v1"),
            "http://127.0.0.1:11434"
        );
        assert_eq!(
            OllamaManager::root_from_provider_url("http://127.0.0.1:11434/v1/"),
            "http://127.0.0.1:11434"
        );
        assert_eq!(
            OllamaManager::root_from_provider_url("http://127.0.0.1:11434"),
            "http://127.0.0.1:11434"
        );
    }
}
