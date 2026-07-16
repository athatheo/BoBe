//! Detection-first: reuse user's existing Ollama on `:11434`; only spawn managed `ollama serve` if absent.

use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::StreamExt;
use serde::Deserialize;
use tokio::sync::{Mutex, watch};
use tracing::{info, warn};

use crate::error::AppError;

const HEALTH_TIMEOUT: Duration = Duration::from_secs(5);
const PULL_TIMEOUT: Duration = Duration::from_hours(2);
const STARTUP_POLL_INTERVAL: Duration = Duration::from_secs(1);
const STARTUP_MAX_ATTEMPTS: u32 = 30;

/// `completed`/`total` are `None` early in the pull (manifest fetch phase).
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

pub(crate) struct OllamaManager {
    http_client: Arc<reqwest::Client>,
    base_url: String,
    /// `Some` only if WE started Ollama; reused daemons stay `None`. The
    /// child is killed on Drop (Tokio process Drop semantics); explicit
    /// shutdown is handled by the daemon's graceful shutdown path which
    /// drops the OllamaInstallService.
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
            .spawn()?;

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
            "Ollama failed to become healthy within {STARTUP_MAX_ATTEMPTS}s"
        )))
    }

    /// Cancellation races both connection establishment and every stream read,
    /// so a stalled Ollama server cannot pin the install until the HTTP timeout.
    pub(crate) async fn pull_model(
        &self,
        name: &str,
        progress_tx: &watch::Sender<PullProgress>,
        mut cancel_rx: watch::Receiver<bool>,
    ) -> Result<(), AppError> {
        let url = format!("{}/api/pull", self.base_url);
        info!(model = name, "ollama_manager.pull_starting");

        let request = self
            .http_client
            .post(&url)
            .json(&serde_json::json!({"name": name}))
            .timeout(PULL_TIMEOUT)
            .send();
        tokio::pin!(request);
        let resp = tokio::select! {
            biased;
            () = wait_for_cancel(&mut cancel_rx) => {
                info!(model = name, "ollama_manager.pull_canceled");
                return Err(AppError::Canceled("Model pull canceled".into()));
            }
            result = &mut request => result
                .map_err(|e| AppError::ServiceUnavailable(format!("ollama pull: {e}")))?,
        };

        if !resp.status().is_success() {
            return Err(AppError::ServiceUnavailable(format!(
                "ollama pull returned {}",
                resp.status()
            )));
        }

        let mut stream = resp.bytes_stream();
        let mut buf = String::new();
        // Throttle in-phase emissions; status transitions and the final
        // success line bypass the throttle so consumers don't miss state
        // changes. Same shape as voice install_service throttle.
        let throttle = Duration::from_millis(250);
        let mut last_emit = Instant::now();
        let mut last_status: Option<String> = None;
        let mut last_percent: Option<u8> = None;

        loop {
            let chunk = tokio::select! {
                biased;
                () = wait_for_cancel(&mut cancel_rx) => {
                    info!(model = name, "ollama_manager.pull_canceled");
                    return Err(AppError::Canceled("Model pull canceled".into()));
                }
                chunk = stream.next() => chunk,
            };
            let Some(chunk) = chunk else { break };

            let bytes =
                chunk.map_err(|e| AppError::ServiceUnavailable(format!("pull stream: {e}")))?;
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

                let status_changed = last_status.as_deref() != Some(status.as_str());
                if status_changed || percent != last_percent || last_emit.elapsed() >= throttle {
                    progress_tx
                        .send(PullProgress {
                            status: status.clone(),
                            completed_bytes: event.completed,
                            total_bytes: event.total,
                            percent,
                        })
                        .ok();
                    last_emit = Instant::now();
                    last_status = Some(status);
                    last_percent = percent;
                }
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

        let tags: TagsResponse = resp.json().await?;
        Ok(tags.models.into_iter().map(|m| m.name).collect())
    }

    /// `http://host/v1` → `http://host` for Ollama's native API root.
    pub(crate) fn root_from_provider_url(provider_url: &str) -> String {
        provider_url
            .trim_end_matches('/')
            .trim_end_matches("/v1")
            .to_string()
    }
}

async fn wait_for_cancel(cancel_rx: &mut watch::Receiver<bool>) {
    if *cancel_rx.borrow() {
        return;
    }
    while cancel_rx.changed().await.is_ok() {
        if *cancel_rx.borrow() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::convert::Infallible;
    use std::sync::Arc;
    use std::time::Duration;

    use axum::Router;
    use axum::body::Bytes;
    use axum::routing::post;
    use tokio::net::TcpListener;
    use tokio::sync::{Notify, watch};

    use super::{OllamaManager, PullProgress};

    #[tokio::test]
    async fn stalled_model_pull_cancels_without_another_chunk() {
        crate::util::tls::install_crypto_provider().expect("TLS provider should install");
        let first_chunk_sent = Arc::new(Notify::new());
        let release_stream = Arc::new(Notify::new());
        let app = Router::new().route(
            "/api/pull",
            post({
                let first_chunk_sent = Arc::clone(&first_chunk_sent);
                let release_stream = Arc::clone(&release_stream);
                move || {
                    let first_chunk_sent = Arc::clone(&first_chunk_sent);
                    let release_stream = Arc::clone(&release_stream);
                    async move {
                        let stream = async_stream::stream! {
                            yield Ok::<_, Infallible>(Bytes::from_static(
                                b"{\"status\":\"pulling\",\"completed\":1,\"total\":2}\n"
                            ));
                            first_chunk_sent.notify_one();
                            release_stream.notified().await;
                        };
                        axum::body::Body::from_stream(stream)
                    }
                }
            }),
        );
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test server should bind");
        let address = listener.local_addr().expect("test address should exist");
        let server = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("test server should run");
        });

        let manager = OllamaManager::new(
            Arc::new(reqwest::Client::new()),
            &format!("http://{address}"),
        );
        let (progress_tx, _progress_rx) = watch::channel(PullProgress::default());
        let (cancel_tx, cancel_rx) = watch::channel(false);
        let pull = tokio::spawn(async move {
            manager
                .pull_model("test-model", &progress_tx, cancel_rx)
                .await
        });

        first_chunk_sent.notified().await;
        cancel_tx.send(true).expect("cancel receiver should exist");
        let error = tokio::time::timeout(Duration::from_secs(1), pull)
            .await
            .expect("cancellation should be prompt")
            .expect("pull task should join")
            .expect_err("pull should be canceled");

        assert!(matches!(error, crate::error::AppError::Canceled(_)));
        release_stream.notify_waiters();
        server.abort();
        drop(server.await);
    }

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
