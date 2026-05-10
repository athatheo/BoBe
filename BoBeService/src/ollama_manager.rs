//! Detection-first: reuse user's existing Ollama on `:11434`; only spawn managed `ollama serve` if absent.

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

/// `completed`/`total` are `None` early in the pull (manifest fetch phase).
#[derive(Debug, Clone, Default)]
pub(crate) struct PullProgress {    pub(crate) status: String,
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

pub(crate) struct OllamaManager {
    http_client: Arc<reqwest::Client>,
    base_url: String,
    /// `Some` only if WE started Ollama; reused daemons stay `None` so `stop()` is a no-op.
    child: Mutex<Option<tokio::process::Child>>,
}

impl OllamaManager {
    /// `base_url` is the root (no `/v1`); Ollama's native API isn't OpenAI-prefixed.
    pub(crate) fn new(http_client: Arc<reqwest::Client>, base_url: &str) -> Self {
        Self {
            http_client,
            base_url: base_url.trim_end_matches('/').to_string(),
            child: Mutex::new(None),
        }
    }

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

        // OLLAMA_HOST + OLLAMA_ORIGINS pin to localhost; mirrors daemon's security posture.
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

    /// `is_canceled` checked between chunks so wizard cancel aborts multi-GB downloads promptly.
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
                return Err(AppError::Canceled("Model pull canceled".into()));
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

        // No `"status":"success"` line — usually a mid-pull disconnect; caller retries.
        Err(AppError::ServiceUnavailable(
            "Ollama pull ended without success confirmation".into(),
        ))
    }

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

    /// No-op if we never spawned the daemon (i.e. reusing user's Ollama).
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

    /// `http://host/v1` → `http://host` for Ollama's native API root.
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
