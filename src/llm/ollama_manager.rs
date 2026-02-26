use std::process::Stdio;
use std::sync::Mutex;

use reqwest::Client;
use serde::Deserialize;
use tracing::{info, warn};

use crate::error::AppError;

/// Manages Ollama process lifecycle and model availability.
///
/// Responsibilities:
/// - Check if Ollama is running
/// - Start Ollama process if not running (when auto_start is enabled)
/// - Pull models if not available locally (when auto_pull is enabled)
pub struct OllamaManager {
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

impl OllamaManager {
    pub fn new(
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
    pub async fn ensure_running(&self) -> Result<(), AppError> {
        if self.health_check().await {
            info!("ollama.already_running");
        } else if self.auto_start {
            info!("ollama.starting");
            self.start_ollama().await?;
            info!("ollama.started");
        } else {
            return Err(AppError::LlmUnavailable(
                "Ollama is not running and auto_start is disabled".into(),
            ));
        }

        if self.has_model(&self.model).await {
            info!(model = %self.model, "ollama.model_available");
        } else if self.auto_pull {
            info!(model = %self.model, "ollama.pulling_model");
            self.pull_model(&self.model).await?;
            info!(model = %self.model, "ollama.model_pulled");
        } else {
            return Err(AppError::LlmUnavailable(format!(
                "Model '{}' not available and auto_pull is disabled",
                self.model
            )));
        }

        Ok(())
    }

    /// Ensure a specific model is available, pulling if needed.
    pub async fn ensure_model(&self, model_name: &str) -> Result<bool, AppError> {
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
        self.pull_model(model_name).await?;
        info!(model = model_name, "ollama.model_pulled");
        Ok(true)
    }

    /// Check if Ollama API is reachable.
    pub async fn health_check(&self) -> bool {
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
    pub async fn has_model(&self, model_name: &str) -> bool {
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
    pub async fn pull_model(&self, model_name: &str) -> Result<(), AppError> {
        let url = format!("{}/api/pull", self.base_url);
        let resp = self
            .client
            .post(&url)
            .json(&serde_json::json!({"name": model_name}))
            .send()
            .await
            .map_err(|e| AppError::LlmUnavailable(format!("Ollama pull request failed: {e}")))?;

        if !resp.status().is_success() {
            return Err(AppError::LlmUnavailable(format!(
                "Ollama pull returned {}",
                resp.status()
            )));
        }

        // Stream the response lines to track progress
        let body = resp.text().await.unwrap_or_default();
        let mut last_logged_pct: i64 = -10;

        for line in body.lines() {
            if line.is_empty() {
                continue;
            }
            if let Ok(progress) = serde_json::from_str::<PullProgress>(line)
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
                    let pct = (completed as f64 / total as f64 * 100.0) as i64;
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

        Ok(())
    }

    /// Start Ollama serve process.
    async fn start_ollama(&self) -> Result<(), AppError> {
        let ollama_path = self
            .binary_path
            .clone()
            .filter(|p| !p.trim().is_empty())
            .ok_or_else(|| {
                AppError::LlmUnavailable(
                    "Ollama binary path is not configured (set BOBE_OLLAMA_BINARY_PATH)".into(),
                )
            })?;

        let child = tokio::process::Command::new(&ollama_path)
            .arg("serve")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .env("OLLAMA_HOST", "127.0.0.1:11434")
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

    /// Stop Ollama if we started it.
    #[allow(unsafe_code)]
    pub async fn stop(&self) {
        let started = *lock_or_recover(&self.started_by_us, "ollama_manager.started_by_us");
        if !started {
            return;
        }

        if let Some(pid) = lock_or_recover(&self.child, "ollama_manager.child").take() {
            info!(pid = pid, "ollama.stopping");
            // SAFETY: libc::kill with a valid PID is safe; we obtained this PID
            // from a child process we spawned and hold under a lock.
            unsafe {
                libc::kill(pid as i32, libc::SIGTERM);
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
