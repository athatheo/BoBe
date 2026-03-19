use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;

use futures::StreamExt;
use reqwest::Client;
use serde::Deserialize;
use tracing::{info, warn};

use crate::error::AppError;

/// Ollama's compiled-in default when no `num_ctx` is configured.
const OLLAMA_DEFAULT_CONTEXT: u32 = 4_096;
const OLLAMA_PULL_TIMEOUT: std::time::Duration = std::time::Duration::from_hours(2);

/// Manages Ollama process lifecycle and model availability.
///
/// Responsibilities:
/// - Check if Ollama is running
/// - Start Ollama process if not running (when auto_start is enabled)
/// - Pull models if not available locally (when auto_pull is enabled)
pub(crate) struct OllamaManager {
    client: Client,
    base_url: String,
    model: String,
    auto_start: bool,
    auto_pull: bool,
    binary_path: Option<String>,
    child: Mutex<Option<u32>>,
    started_by_us: Mutex<bool>,
}

#[derive(Debug, Deserialize)]
struct TagsResponse {
    models: Vec<ModelInfo>,
}

#[derive(Debug, Deserialize)]
struct ModelInfo {
    name: String,
}

#[derive(Debug, Deserialize)]
struct PullProgress {
    status: Option<String>,
    completed: Option<u64>,
    total: Option<u64>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ShowResponse {
    /// Runtime parameters as a newline-delimited key-value string.
    #[serde(default)]
    parameters: Option<String>,
    /// Model metadata containing context_length and other model info.
    #[serde(default)]
    model_info: Option<serde_json::Value>,
}

impl OllamaManager {
    pub(crate) fn new(
        client: Client,
        base_url: &str,
        model: &str,
        auto_start: bool,
        auto_pull: bool,
        binary_path: Option<String>,
    ) -> Self {
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_owned(),
            model: model.to_owned(),
            auto_start,
            auto_pull,
            binary_path,
            child: Mutex::new(None),
            started_by_us: Mutex::new(false),
        }
    }

    /// Ensure Ollama is running and the configured model is available.
    pub(crate) async fn ensure_running(&self) -> Result<(), AppError> {
        self.ensure_daemon_running(None, self.auto_start).await?;

        if self.has_model(&self.model).await {
            info!(model = %self.model, "ollama.model_available");
        } else if self.auto_pull {
            info!(model = %self.model, "ollama.pulling_model");
            self.pull_model(&self.model, || false).await?;
            info!(model = %self.model, "ollama.model_pulled");
        } else {
            return Err(AppError::LlmUnavailable(format!(
                "Model '{}' not available and auto_pull is disabled",
                self.model
            )));
        }

        Ok(())
    }

    /// Ensure only the Ollama daemon is reachable, optionally starting it with
    /// a caller-provided binary path.
    pub(crate) async fn ensure_daemon_running(
        &self,
        binary_path: Option<&Path>,
        auto_start: bool,
    ) -> Result<(), AppError> {
        if self.health_check().await {
            info!("ollama.already_running");
            return Ok(());
        }

        if !auto_start {
            return Err(AppError::LlmUnavailable(
                "Ollama is not running and auto_start is disabled".into(),
            ));
        }

        info!("ollama.starting");
        self.start_ollama(binary_path).await?;
        info!("ollama.started");
        Ok(())
    }

    /// Ensure a specific model is available, pulling if needed.
    pub(crate) async fn ensure_model(&self, model_name: &str) -> Result<bool, AppError> {
        if self.has_model(model_name).await {
            info!(model = model_name, "ollama.model_available");
            return Ok(true);
        }

        if !self.auto_pull {
            warn!(
                model = model_name,
                "ollama.model_not_available, auto_pull disabled"
            );
            return Ok(false);
        }

        info!(model = model_name, "ollama.pulling_model");
        self.pull_model(model_name, || false).await?;
        info!(model = model_name, "ollama.model_pulled");
        Ok(true)
    }

    /// Query Ollama for the effective context window of a model.
    ///
    /// Resolution order:
    /// 1. `num_ctx` from runtime `parameters` (the *actual* window in use)
    /// 2. `context_length` from `model_info` (model's max capability)
    /// 3. [`OLLAMA_DEFAULT_CONTEXT`] (Ollama's built-in default)
    pub(crate) async fn get_context_window(&self, model: &str) -> u32 {
        let url = format!("{}/api/show", self.base_url);
        let resp = match self
            .client
            .post(&url)
            .json(&serde_json::json!({"name": model}))
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                warn!(status = %r.status(), "ollama.show_request_failed");
                return OLLAMA_DEFAULT_CONTEXT;
            }
            Err(e) => {
                warn!(error = %e, "ollama.show_request_error");
                return OLLAMA_DEFAULT_CONTEXT;
            }
        };

        let show: ShowResponse = match resp.json().await {
            Ok(s) => s,
            Err(e) => {
                warn!(error = %e, "ollama.show_parse_error");
                return OLLAMA_DEFAULT_CONTEXT;
            }
        };

        // 1. Check runtime parameters for num_ctx
        if let Some(ref params) = show.parameters
            && let Some(ctx) = parse_num_ctx(params)
        {
            info!(
                model = model,
                num_ctx = ctx,
                "ollama.context_window_from_parameters"
            );
            return ctx;
        }

        // 2. Check model_info for context_length
        if let Some(ref info) = show.model_info
            && let Some(ctx) = extract_context_length(info)
        {
            info!(
                model = model,
                context_length = ctx,
                "ollama.context_window_from_model_info"
            );
            return ctx;
        }

        info!(
            model = model,
            default = OLLAMA_DEFAULT_CONTEXT,
            "ollama.context_window_using_default"
        );
        OLLAMA_DEFAULT_CONTEXT
    }

    /// Check if Ollama API is reachable.
    pub(crate) async fn health_check(&self) -> bool {
        let url = format!("{}/api/tags", self.base_url);
        match self
            .client
            .get(&url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
        {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    /// Check if a model is available locally.
    pub(crate) async fn has_model(&self, model_name: &str) -> bool {
        let url = format!("{}/api/tags", self.base_url);
        let resp = match self
            .client
            .get(&url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => r,
            _ => return false,
        };

        let tags: TagsResponse = match resp.json().await {
            Ok(t) => t,
            Err(_) => return false,
        };

        let has_tag = model_name.contains(':');
        let base_name = model_name.split(':').next().unwrap_or(model_name);

        for model in &tags.models {
            if has_tag {
                if model.name == model_name {
                    return true;
                }
            } else if model.name == model_name || model.name.starts_with(&format!("{base_name}:")) {
                return true;
            }
        }

        false
    }

    /// Pull a model from the Ollama registry.
    ///
    /// Streams the NDJSON response line-by-line and checks `is_canceled` between
    /// lines, allowing the caller to abort a multi-GB download promptly.
    pub(crate) async fn pull_model(
        &self,
        model_name: &str,
        is_canceled: impl Fn() -> bool,
    ) -> Result<(), AppError> {
        ensure_ollama_key_pair();

        let url = format!("{}/api/pull", self.base_url);
        let resp = self
            .client
            .post(&url)
            .json(&serde_json::json!({"name": model_name}))
            .timeout(OLLAMA_PULL_TIMEOUT)
            .send()
            .await
            .map_err(|e| AppError::LlmUnavailable(format!("Ollama pull request failed: {e}")))?;

        if !resp.status().is_success() {
            return Err(AppError::LlmUnavailable(format!(
                "Ollama pull returned {}",
                resp.status()
            )));
        }

        // Stream response line-by-line to track progress and support cancellation.
        let mut stream = resp.bytes_stream();
        let mut buf = String::new();
        let mut last_logged_pct: i64 = -10;

        while let Some(chunk) = stream.next().await {
            if is_canceled() {
                info!(model = model_name, "ollama.pull_canceled");
                return Err(AppError::LlmUnavailable("Model pull canceled".to_string()));
            }

            let bytes =
                chunk.map_err(|e| AppError::LlmUnavailable(format!("Stream read error: {e}")))?;
            buf.push_str(&String::from_utf8_lossy(&bytes));

            // Process complete lines from the buffer
            while let Some(newline_pos) = buf.find('\n') {
                let line = buf[..newline_pos].trim().to_string();
                buf.drain(..=newline_pos);

                if line.is_empty() {
                    continue;
                }
                if let Ok(progress) = serde_json::from_str::<PullProgress>(&line)
                    && let Some(status) = &progress.status
                {
                    if status == "success" {
                        return Ok(());
                    }
                    if let Some(err) = &progress.error {
                        return Err(AppError::LlmUnavailable(format!(
                            "Ollama pull error: {err}"
                        )));
                    }
                    if let (Some(completed), Some(total)) = (progress.completed, progress.total)
                        && total > 0
                    {
                        let pct = (completed as f64 / total as f64 * 100.0).min(100.0) as i64;
                        if pct >= last_logged_pct + 10 {
                            last_logged_pct = (pct / 10) * 10;
                            info!(
                                model = model_name,
                                progress = format!("{pct}%"),
                                "ollama.pull_progress"
                            );
                        }
                    }
                }
            }
        }

        Err(AppError::LlmUnavailable(
            "Model pull ended without success confirmation (possible network interruption)"
                .to_string(),
        ))
    }

    /// Start Ollama serve process.
    async fn start_ollama(&self, binary_path: Option<&Path>) -> Result<(), AppError> {
        let ollama_path = self.resolve_binary_path(binary_path)?;

        let ollama_host = self
            .base_url
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_end_matches('/');
        let child = tokio::process::Command::new(&ollama_path)
            .arg("serve")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .env("OLLAMA_HOST", ollama_host)
            .env("OLLAMA_ORIGINS", "http://127.0.0.1:*")
            .spawn()
            .map_err(|e| AppError::LlmUnavailable(format!("Failed to start ollama: {e}")))?;

        if let Some(pid) = child.id() {
            *lock_or_recover(&self.child, "ollama_manager.child") = Some(pid);
        }
        *lock_or_recover(&self.started_by_us, "ollama_manager.started_by_us") = true;

        // Wait for Ollama to be ready (up to 30 seconds)
        for _ in 0..30 {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            if self.health_check().await {
                return Ok(());
            }
        }

        Err(AppError::LlmUnavailable(
            "Ollama startup timeout (30s)".into(),
        ))
    }

    fn resolve_binary_path(&self, override_path: Option<&Path>) -> Result<PathBuf, AppError> {
        override_path
            .map(Path::to_path_buf)
            .or_else(|| {
                self.binary_path
                    .clone()
                    .filter(|p| !p.trim().is_empty())
                    .map(PathBuf::from)
            })
            .ok_or_else(|| {
                AppError::LlmUnavailable(
                    "Ollama binary path is not configured (set BOBE_OLLAMA_BINARY_PATH)".into(),
                )
            })
    }

    /// Stop Ollama if we started it.
    #[allow(unsafe_code)]
    pub(crate) async fn stop(&self) {
        let started = *lock_or_recover(&self.started_by_us, "ollama_manager.started_by_us");
        if !started {
            return;
        }

        if let Some(pid) = lock_or_recover(&self.child, "ollama_manager.child").take() {
            info!(pid = pid, "ollama.stopping");
            // SAFETY: libc::kill with a valid PID is safe; we obtained this PID
            // from a child process we spawned and hold under a lock.
            unsafe {
                libc::kill(i32::try_from(pid).unwrap_or(-1), libc::SIGTERM);
            }
            *lock_or_recover(&self.started_by_us, "ollama_manager.started_by_us") = false;
            info!("ollama.stopped");
        }
    }
}

fn lock_or_recover<'a, T>(
    mutex: &'a Mutex<T>,
    lock_name: &'static str,
) -> std::sync::MutexGuard<'a, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            warn!(lock = lock_name, "mutex poisoned, recovering");
            poisoned.into_inner()
        }
    }
}

/// Ensure `~/.ollama/id_ed25519` exists. Ollama requires this key pair to
/// authenticate with the model registry; without it every pull fails with
/// "open ~/.ollama/id_ed25519: no such file or directory".
pub(crate) fn ensure_ollama_key_pair() {
    let ollama_dir = match dirs::home_dir() {
        Some(home) => home.join(".ollama"),
        None => return,
    };
    let key_path = ollama_dir.join("id_ed25519");
    if key_path.exists() {
        return;
    }
    if std::fs::create_dir_all(&ollama_dir).is_err() {
        warn!("ollama.failed_to_create_dir: {}", ollama_dir.display());
        return;
    }
    let status = std::process::Command::new("ssh-keygen")
        .args(["-t", "ed25519", "-f"])
        .arg(&key_path)
        .args(["-N", "", "-q"])
        .status();
    match status {
        Ok(s) if s.success() => info!("ollama.generated_registry_key"),
        Ok(s) => warn!("ollama.keygen_failed: exit {s}"),
        Err(e) => warn!("ollama.keygen_failed: {e}"),
    }
}

/// Parse `num_ctx` from Ollama's newline-delimited parameter string.
///
/// Example input: `"num_ctx 8192\ntemperature 0.7\n"`
fn parse_num_ctx(parameters: &str) -> Option<u32> {
    for line in parameters.lines() {
        let trimmed = line.trim();
        if let Some(value_str) = trimmed.strip_prefix("num_ctx")
            && let Ok(v) = value_str.trim().parse::<u32>()
        {
            return Some(v);
        }
    }
    None
}

/// Extract `context_length` from model_info JSON.
///
/// Ollama stores this under a model-family-specific key, e.g.
/// `model_info.llama.context_length` or `model_info.qwen2.context_length`.
/// We scan all top-level objects for a `context_length` field.
fn extract_context_length(model_info: &serde_json::Value) -> Option<u32> {
    let obj = model_info.as_object()?;
    for (key, value) in obj {
        // Direct context_length at top level
        if let Some(n) = value.as_u64()
            && key.contains("context_length")
        {
            return u32::try_from(n).ok();
        }
        // Nested within a family object
        if let Some(inner) = value.as_object()
            && let Some(ctx) = inner
                .get("context_length")
                .and_then(serde_json::Value::as_u64)
        {
            return u32::try_from(ctx).ok();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::OllamaManager;

    #[test]
    fn resolve_binary_path_prefers_override() {
        let manager = OllamaManager::new(
            reqwest::Client::new(),
            "http://localhost:11434",
            "qwen3:4b",
            true,
            true,
            Some("/tmp/configured-ollama".into()),
        );

        let path = manager
            .resolve_binary_path(Some(std::path::Path::new("/tmp/override-ollama")))
            .expect("override path should resolve");

        assert_eq!(path, std::path::PathBuf::from("/tmp/override-ollama"));
    }

    #[test]
    fn resolve_binary_path_falls_back_to_configured_path() {
        let manager = OllamaManager::new(
            reqwest::Client::new(),
            "http://localhost:11434",
            "qwen3:4b",
            true,
            true,
            Some("/tmp/configured-ollama".into()),
        );

        let path = manager
            .resolve_binary_path(None)
            .expect("configured path should resolve");

        assert_eq!(path, std::path::PathBuf::from("/tmp/configured-ollama"));
    }
}
