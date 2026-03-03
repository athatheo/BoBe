//! Setup service — orchestrates local and cloud provisioning pipelines.
//!
//! Extracted from the setup handler to keep handler files focused on HTTP
//! concerns while this module owns the multi-step provisioning logic.

use std::collections::HashMap;
use std::sync::Arc;

use arc_swap::ArcSwap;
use secrecy::SecretString;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::api::handlers::setup::{
    JobStatus, SetupJobState, SetupRequest, StepProgress, StepStatus,
};
use crate::app_state::AppState;
use crate::config::LlmBackend;
use crate::error::AppError;
use crate::llm::factory::LlmProviderFactory;

// ── Shared job state ────────────────────────────────────────────────────────

pub(crate) type SharedJobState = Arc<RwLock<Option<SetupJobState>>>;

/// Global singleton for the setup job state.
/// We use a static `OnceLock` since we can't modify `AppState`'s struct definition.
static SETUP_JOB: std::sync::OnceLock<SharedJobState> = std::sync::OnceLock::new();

pub(crate) fn job_state() -> &'static SharedJobState {
    SETUP_JOB.get_or_init(|| Arc::new(RwLock::new(None)))
}

// ── Model tier mapping ──────────────────────────────────────────────────────

/// Disk space estimate in bytes for a given local tier.
/// Used by both `get_options` (client-facing) and `run_local_setup` (validation).
pub(crate) fn tier_disk_estimate(tier: &str) -> u64 {
    match tier {
        "small" => 6_000_000_000,
        "medium" => 11_000_000_000,
        _ => 15_000_000_000,
    }
}

struct TierModels {
    text: &'static str,
    vision: &'static str,
}

fn tier_models(tier: &str) -> TierModels {
    match tier {
        "small" => TierModels {
            text: "qwen3:4b",
            vision: "qwen3-vl:4b",
        },
        "medium" => TierModels {
            text: "qwen3:8b",
            vision: "qwen3-vl:8b",
        },
        _ => TierModels {
            text: "qwen3:14b",
            vision: "qwen3-vl:8b",
        },
    }
}

// ── Step helpers ─────────────────────────────────────────────────────────────

async fn update_step(step_id: &str, status: StepStatus, message: Option<String>) {
    let mut lock = job_state().write().await;
    if let Some(ref mut job) = *lock {
        if let Some(s) = job.steps.iter_mut().find(|s| s.id == step_id) {
            s.status = status;
            s.message = message;
            s.progress = None;
        }
        // Update current_step to the first in-progress step
        job.current_step = job
            .steps
            .iter()
            .find(|s| s.status == StepStatus::InProgress)
            .map(|s| s.id.clone());
    }
}

async fn update_step_progress(step_id: &str, progress: StepProgress) {
    let mut lock = job_state().write().await;
    if let Some(ref mut job) = *lock
        && let Some(s) = job.steps.iter_mut().find(|s| s.id == step_id)
    {
        s.progress = Some(progress);
    }
}

async fn finish_job(status: JobStatus, error: Option<String>) {
    let mut lock = job_state().write().await;
    if let Some(ref mut job) = *lock {
        job.status = status;
        job.error = error;
        job.current_step = None;
    }
}

async fn is_canceled() -> bool {
    let lock = job_state().read().await;
    lock.as_ref()
        .is_some_and(|j| j.status == JobStatus::Canceled)
}

// ── Job runners ─────────────────────────────────────────────────────────────

pub(crate) async fn run_local_setup(state: Arc<AppState>, body: SetupRequest) {
    {
        let mut lock = job_state().write().await;
        if let Some(ref mut job) = *lock {
            job.status = JobStatus::InProgress;
        }
    }

    let tier = body.tier.as_deref().unwrap_or("large");
    let models = tier_models(tier);

    // Step 1: Validate data directory and disk space
    update_step("validate", StepStatus::InProgress, None).await;
    let data_dir = state.config().resolved_data_dir();
    if let Err(e) = tokio::fs::create_dir_all(&data_dir).await {
        update_step(
            "validate",
            StepStatus::Failed,
            Some(format!("Cannot create data directory: {e}")),
        )
        .await;
        finish_job(JobStatus::Failed, Some(e.to_string())).await;
        return;
    }

    // Check available disk space against the tier's estimate
    let required_bytes = tier_disk_estimate(tier);
    if let Some(available) = available_disk_space(&data_dir).await
        && available < required_bytes
    {
        let needed_gb = required_bytes / 1_000_000_000;
        let avail_gb = available / 1_000_000_000;
        let msg =
            format!("Not enough disk space: ~{needed_gb} GB required, {avail_gb} GB available");
        update_step("validate", StepStatus::Failed, Some(msg.clone())).await;
        finish_job(JobStatus::Failed, Some(msg)).await;
        return;
    }

    update_step(
        "validate",
        StepStatus::Succeeded,
        Some("Data directory ready".into()),
    )
    .await;

    if is_canceled().await {
        finish_job(JobStatus::Canceled, None).await;
        return;
    }

    // Step 2: Ensure Ollama engine
    update_step("engine", StepStatus::InProgress, None).await;
    let (progress_tx, mut progress_rx) =
        tokio::sync::watch::channel(crate::binary_manager::DownloadProgress {
            current_bytes: 0,
            total_bytes: None,
            percent: None,
        });

    // Spawn a task to relay progress updates
    let progress_relay = tokio::spawn(async move {
        while progress_rx.changed().await.is_ok() {
            let p = progress_rx.borrow().clone();
            update_step_progress(
                "engine",
                StepProgress {
                    percent: p.percent,
                    current_bytes: Some(p.current_bytes),
                    total_bytes: p.total_bytes,
                },
            )
            .await;
        }
    });

    let binary_path = match state.binary_manager.ensure_ollama(&progress_tx).await {
        Ok(path) => {
            update_step(
                "engine",
                StepStatus::Succeeded,
                Some(format!("Ollama at {}", path.display())),
            )
            .await;
            path
        }
        Err(e) => {
            update_step("engine", StepStatus::Failed, Some(e.to_string())).await;
            finish_job(JobStatus::Failed, Some(e.to_string())).await;
            return;
        }
    };
    drop(progress_tx);
    let _ = progress_relay.await;

    if is_canceled().await {
        finish_job(JobStatus::Canceled, None).await;
        return;
    }

    // Ensure Ollama is running
    if let Err(e) = state.ollama_manager.ensure_running().await {
        warn!(error = %e, "setup.ollama_start_failed");
        // Not fatal — user may have system Ollama
    }

    // Step 3: Pull text model
    update_step(
        "text_model",
        StepStatus::InProgress,
        Some(format!("Pulling {}", models.text)),
    )
    .await;
    match pull_model(&state, models.text).await {
        Ok(()) => {
            update_step(
                "text_model",
                StepStatus::Succeeded,
                Some(format!("{} ready", models.text)),
            )
            .await;
        }
        Err(e) => {
            update_step("text_model", StepStatus::Failed, Some(e.to_string())).await;
            finish_job(JobStatus::Failed, Some(e.to_string())).await;
            return;
        }
    }

    if is_canceled().await {
        finish_job(JobStatus::Canceled, None).await;
        return;
    }

    // Step 4: Pull vision model
    update_step(
        "vision_model",
        StepStatus::InProgress,
        Some(format!("Pulling {}", models.vision)),
    )
    .await;
    match pull_model(&state, models.vision).await {
        Ok(()) => {
            update_step(
                "vision_model",
                StepStatus::Succeeded,
                Some(format!("{} ready", models.vision)),
            )
            .await;
        }
        Err(e) => {
            // Vision model failure is non-fatal
            warn!(error = %e, "setup.vision_model_pull_failed");
            update_step(
                "vision_model",
                StepStatus::Failed,
                Some(format!("Vision model pull failed (non-fatal): {e}")),
            )
            .await;
        }
    }

    if is_canceled().await {
        finish_job(JobStatus::Canceled, None).await;
        return;
    }

    // Step 5: Pull embedding model
    let embedding_model = "BAAI/bge-small-en-v1.5";
    update_step(
        "embedding_model",
        StepStatus::InProgress,
        Some(format!("Pulling {embedding_model}")),
    )
    .await;
    match pull_model(&state, embedding_model).await {
        Ok(()) => {
            update_step(
                "embedding_model",
                StepStatus::Succeeded,
                Some(format!("{embedding_model} ready")),
            )
            .await;
        }
        Err(e) => {
            update_step("embedding_model", StepStatus::Failed, Some(e.to_string())).await;
            finish_job(JobStatus::Failed, Some(e.to_string())).await;
            return;
        }
    }

    if is_canceled().await {
        finish_job(JobStatus::Canceled, None).await;
        return;
    }

    // Step 6: Warmup embedding
    update_step(
        "embedding_warmup",
        StepStatus::InProgress,
        Some("Loading embedding model into memory...".into()),
    )
    .await;
    match state.embedding_provider.embed("warmup").await {
        Ok(_) => {
            update_step(
                "embedding_warmup",
                StepStatus::Succeeded,
                Some("Embedding model loaded".into()),
            )
            .await;
        }
        Err(e) => {
            warn!(error = %e, "setup.embedding_warmup_failed");
            update_step(
                "embedding_warmup",
                StepStatus::Failed,
                Some(format!("Warmup failed (non-fatal): {e}")),
            )
            .await;
        }
    }

    // Step 7: Persist config
    update_step("persist", StepStatus::InProgress, None).await;
    let mut changes = HashMap::new();
    changes.insert(
        "llm.backend".to_string(),
        serde_json::Value::String("ollama".into()),
    );
    changes.insert(
        "ollama.model".to_string(),
        serde_json::Value::String(models.text.into()),
    );
    changes.insert(
        "ollama.binary_path".to_string(),
        serde_json::Value::String(binary_path.to_string_lossy().into()),
    );
    changes.insert(
        "vision.backend".to_string(),
        serde_json::Value::String("ollama".into()),
    );
    changes.insert(
        "vision.ollama_model".to_string(),
        serde_json::Value::String(models.vision.into()),
    );
    changes.insert("setup_completed".to_string(), serde_json::Value::Bool(true));

    let update = state.config_manager.update(&changes);
    if update.persist_failed {
        update_step(
            "persist",
            StepStatus::Failed,
            Some("Failed to persist configuration".into()),
        )
        .await;
        finish_job(
            JobStatus::Failed,
            Some("Failed to persist configuration".into()),
        )
        .await;
        return;
    }
    update_step(
        "persist",
        StepStatus::Succeeded,
        Some("Configuration saved".into()),
    )
    .await;

    info!("setup.local_complete");
    finish_job(JobStatus::Succeeded, None).await;
}

pub(crate) async fn run_cloud_setup(state: Arc<AppState>, body: SetupRequest) {
    {
        let mut lock = job_state().write().await;
        if let Some(ref mut job) = *lock {
            job.status = JobStatus::InProgress;
        }
    }

    let provider = body.provider.as_deref().unwrap_or("openai");
    update_step("validate", StepStatus::InProgress, None).await;

    match provider {
        "openai" => run_openai_setup(&state, &body).await,
        "azure_openai" => run_azure_setup(&state, &body).await,
        other => {
            update_step(
                "validate",
                StepStatus::Failed,
                Some(format!("Unknown provider: {other}")),
            )
            .await;
            finish_job(
                JobStatus::Failed,
                Some(format!("Unknown provider: {other}")),
            )
            .await;
        }
    }
}

async fn run_openai_setup(state: &Arc<AppState>, body: &SetupRequest) {
    let Some(api_key) = body.api_key.as_ref().filter(|k| !k.is_empty()).cloned() else {
        fail_step(
            "validate",
            "API key is required",
            "API key is required for OpenAI",
        )
        .await;
        return;
    };

    match state
        .http_client
        .get("https://api.openai.com/v1/models")
        .header("Authorization", format!("Bearer {api_key}"))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            update_step(
                "validate",
                StepStatus::Succeeded,
                Some("API key valid".into()),
            )
            .await;
        }
        Ok(resp) => {
            let msg = format!("API key validation failed: HTTP {}", resp.status());
            fail_step("validate", &msg, &msg).await;
            return;
        }
        Err(e) => {
            let msg = format!("Cannot reach OpenAI: {e}");
            fail_step("validate", &msg, &msg).await;
            return;
        }
    }

    // Test embedding
    update_step(
        "embedding_warmup",
        StepStatus::InProgress,
        Some("Testing embedding endpoint...".into()),
    )
    .await;

    let model = body
        .model
        .clone()
        .unwrap_or_else(|| "gpt-4o-mini".to_string());

    match test_openai_embedding(state, &api_key, &model).await {
        Ok(()) => {
            update_step(
                "embedding_warmup",
                StepStatus::Succeeded,
                Some("Embedding endpoint working".into()),
            )
            .await;
        }
        Err(e) => {
            warn!(error = %e, "setup.embedding_test_failed");
            let msg = format!("Embedding test failed: {e}");
            fail_step("embedding_warmup", &msg, &msg).await;
            return;
        }
    }

    // Persist
    let mut changes = HashMap::new();
    changes.insert(
        "llm.backend".to_string(),
        serde_json::Value::String("openai".into()),
    );
    changes.insert(
        "llm.openai_api_key".to_string(),
        serde_json::Value::String(api_key),
    );
    changes.insert(
        "llm.openai_model".to_string(),
        serde_json::Value::String(model),
    );
    changes.insert("setup_completed".to_string(), serde_json::Value::Bool(true));
    if persist_config(state, changes).await {
        info!("setup.openai_complete");
        finish_job(JobStatus::Succeeded, None).await;
    }
}

async fn run_azure_setup(state: &Arc<AppState>, body: &SetupRequest) {
    let Some(api_key) = body.api_key.as_ref().filter(|k| !k.is_empty()).cloned() else {
        fail_step("validate", "API key is required", "API key required").await;
        return;
    };
    let Some(endpoint) = body.endpoint.as_ref().filter(|e| !e.is_empty()).cloned() else {
        fail_step("validate", "Endpoint is required", "Endpoint required").await;
        return;
    };
    let Some(deployment) = body.deployment.as_ref().filter(|d| !d.is_empty()).cloned() else {
        fail_step(
            "validate",
            "Deployment name is required",
            "Deployment required",
        )
        .await;
        return;
    };

    // Test Azure endpoint
    let test_url = format!(
        "{}/openai/deployments/{}/chat/completions?api-version=2024-02-15-preview",
        endpoint.trim_end_matches('/'),
        deployment
    );
    match state
        .http_client
        .post(&test_url)
        .header("api-key", &api_key)
        .json(&serde_json::json!({"messages": [{"role": "user", "content": "test"}], "max_tokens": 1}))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() || resp.status().as_u16() == 400 => {
            update_step(
                "validate",
                StepStatus::Succeeded,
                Some("Azure endpoint validated".into()),
            )
            .await;
        }
        Ok(resp) => {
            let msg = format!("Azure validation failed: HTTP {}", resp.status());
            fail_step("validate", &msg, &msg).await;
            return;
        }
        Err(e) => {
            let msg = format!("Cannot reach Azure endpoint: {e}");
            fail_step("validate", &msg, &msg).await;
            return;
        }
    }

    // Test embedding
    update_step(
        "embedding_warmup",
        StepStatus::InProgress,
        Some("Testing embedding...".into()),
    )
    .await;
    match test_azure_embedding(state, &endpoint, &api_key, &deployment).await {
        Ok(()) => {
            update_step(
                "embedding_warmup",
                StepStatus::Succeeded,
                Some("Embedding working".into()),
            )
            .await;
        }
        Err(e) => {
            let msg = format!("Embedding failed: {e}");
            fail_step("embedding_warmup", &msg, &msg).await;
            return;
        }
    }

    // Persist
    let mut changes = HashMap::new();
    changes.insert(
        "llm.backend".to_string(),
        serde_json::Value::String("azure_openai".into()),
    );
    changes.insert(
        "llm.azure_openai_endpoint".to_string(),
        serde_json::Value::String(endpoint),
    );
    changes.insert(
        "llm.azure_openai_api_key".to_string(),
        serde_json::Value::String(api_key),
    );
    changes.insert(
        "llm.azure_openai_deployment".to_string(),
        serde_json::Value::String(deployment),
    );
    changes.insert("setup_completed".to_string(), serde_json::Value::Bool(true));
    if persist_config(state, changes).await {
        info!("setup.azure_complete");
        finish_job(JobStatus::Succeeded, None).await;
    }
}

/// Shared helper: mark a step failed and fail the whole job.
async fn fail_step(step_id: &str, step_msg: &str, job_msg: &str) {
    update_step(step_id, StepStatus::Failed, Some(step_msg.to_owned())).await;
    finish_job(JobStatus::Failed, Some(job_msg.to_owned())).await;
}

/// Shared helper: persist config changes and update the persist step.
/// Returns `true` on success.
async fn persist_config(
    state: &Arc<AppState>,
    changes: HashMap<String, serde_json::Value>,
) -> bool {
    update_step("persist", StepStatus::InProgress, None).await;
    let update = state.config_manager.update(&changes);
    if update.persist_failed {
        fail_step(
            "persist",
            "Failed to persist configuration",
            "Failed to persist configuration",
        )
        .await;
        return false;
    }
    update_step(
        "persist",
        StepStatus::Succeeded,
        Some("Configuration saved".into()),
    )
    .await;
    true
}

// ── Helper functions ────────────────────────────────────────────────────────

async fn test_openai_embedding(
    state: &Arc<AppState>,
    api_key: &str,
    model: &str,
) -> Result<(), AppError> {
    let current = state.config();
    let mut candidate = (**current).clone();
    drop(current);

    candidate.llm.backend = LlmBackend::Openai;
    candidate.llm.openai_api_key = SecretString::from(api_key.to_string());
    candidate.llm.openai_model = model.to_string();

    let factory = LlmProviderFactory::new(
        state.http_client.clone(),
        Arc::new(ArcSwap::from_pointee(candidate)),
    );
    let embedding = factory.create_embedding()?;
    embedding.embed("warmup").await.map(|_| ())
}

async fn test_azure_embedding(
    state: &Arc<AppState>,
    endpoint: &str,
    api_key: &str,
    deployment: &str,
) -> Result<(), AppError> {
    let current = state.config();
    let mut candidate = (**current).clone();
    drop(current);

    candidate.llm.backend = LlmBackend::AzureOpenai;
    candidate.llm.azure_openai_endpoint = endpoint.to_string();
    candidate.llm.azure_openai_api_key = SecretString::from(api_key.to_string());
    candidate.llm.azure_openai_deployment = deployment.to_string();

    let factory = LlmProviderFactory::new(
        state.http_client.clone(),
        Arc::new(ArcSwap::from_pointee(candidate)),
    );
    let embedding = factory.create_embedding()?;
    embedding.embed("warmup").await.map(|_| ())
}

async fn pull_model(state: &Arc<AppState>, model: &str) -> Result<(), AppError> {
    // Check if model already exists (idempotent)
    if state.ollama_manager.has_model(model).await {
        info!(model, "setup.model_already_exists");
        return Ok(());
    }

    state
        .ollama_manager
        .pull_model(model, || {
            // Check the shared job state for cancellation.
            // Use try_read to avoid blocking — if the lock is held, assume not canceled.
            SETUP_JOB
                .get()
                .and_then(|s| s.try_read().ok())
                .and_then(|lock| lock.as_ref().map(|j| j.status == JobStatus::Canceled))
                .unwrap_or(false)
        })
        .await?;
    Ok(())
}

/// Returns available bytes on the filesystem containing `path`, or None on error.
async fn available_disk_space(path: &std::path::Path) -> Option<u64> {
    // -P: POSIX format (prevents long filesystem names from wrapping lines)
    // -k: output in 1K blocks
    let output = tokio::process::Command::new("df")
        .args(["-Pk"])
        .arg(path)
        .output()
        .await
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    // POSIX df output: Filesystem 1024-blocks Used Available Capacity Mounted
    let line = stdout.lines().nth(1)?;
    let available_kb: u64 = line.split_whitespace().nth(3)?.parse().ok()?;
    Some(available_kb * 1024)
}
