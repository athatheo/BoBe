//! Setup job runner — single POST creates a background job that provisions everything.
//!
//! Consolidates onboarding into a single idempotent setup job pipeline.

use std::collections::HashMap;
use std::sync::Arc;

use arc_swap::ArcSwap;
use axum::Json;
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::app_state::AppState;
use crate::config::LlmBackend;
use crate::error::AppError;
use crate::llm::factory::LlmProviderFactory;

// ── Onboarding options ─────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct LocalTier {
    pub id: String,
    pub label: String,
    pub description: String,
    pub disk_estimate_bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct CloudProvider {
    pub id: String,
    pub label: String,
    pub requires: Vec<String>,
    pub models: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recommended: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OnboardingOptions {
    pub local_tiers: Vec<LocalTier>,
    pub cloud_providers: Vec<CloudProvider>,
}

/// GET /api/onboarding/options
pub async fn get_options(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<OnboardingOptions>, AppError> {
    Ok(Json(OnboardingOptions {
        local_tiers: vec![
            LocalTier {
                id: "small".into(),
                label: "Small (4B)".into(),
                description: "Fast, low resource usage. Good for quick interactions.".into(),
                disk_estimate_bytes: 6_000_000_000,
            },
            LocalTier {
                id: "medium".into(),
                label: "Medium (8B)".into(),
                description: "Balanced performance and quality.".into(),
                disk_estimate_bytes: 11_000_000_000,
            },
            LocalTier {
                id: "large".into(),
                label: "Large (14B)".into(),
                description: "Best quality, requires more resources.".into(),
                disk_estimate_bytes: 15_000_000_000,
            },
        ],
        cloud_providers: vec![
            CloudProvider {
                id: "openai".into(),
                label: "OpenAI".into(),
                requires: vec!["api_key".into()],
                models: vec!["gpt-4o-mini".into(), "gpt-4o".into()],
                recommended: Some("gpt-4o-mini".into()),
            },
            CloudProvider {
                id: "azure_openai".into(),
                label: "Azure OpenAI".into(),
                requires: vec!["api_key".into(), "endpoint".into(), "deployment".into()],
                models: vec![],
                recommended: None,
            },
        ],
    }))
}

// ── Setup job types ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    InProgress,
    Succeeded,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize)]
pub struct StepProgress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SetupStep {
    pub id: String,
    pub status: StepStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<StepProgress>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Pending,
    InProgress,
    Succeeded,
    Failed,
    Canceled,
}

#[derive(Debug, Clone, Serialize)]
pub struct SetupJobState {
    pub job_id: String,
    pub status: JobStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_step: Option<String>,
    pub steps: Vec<SetupStep>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SetupRequest {
    pub mode: String,
    #[serde(default)]
    pub tier: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub deployment: Option<String>,
}

// ── Shared job state ───────────────────────────────────────────────────────

type SharedJobState = Arc<RwLock<Option<SetupJobState>>>;

/// Extension trait to store job state in AppState.
/// We use a static OnceCell since we can't modify AppState's struct definition.
static SETUP_JOB: std::sync::OnceLock<SharedJobState> = std::sync::OnceLock::new();

fn job_state() -> &'static SharedJobState {
    SETUP_JOB.get_or_init(|| Arc::new(RwLock::new(None)))
}

// ── Model tier mapping ─────────────────────────────────────────────────────

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

// ── Handlers ───────────────────────────────────────────────────────────────

/// POST /api/onboarding/setup — 202 Accepted, spawns background job.
pub async fn create_setup_job(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SetupRequest>,
) -> Result<(axum::http::StatusCode, Json<SetupJobState>), AppError> {
    // Validate mode before taking the lock.
    let steps = match body.mode.as_str() {
        "local" => vec![
            step("validate"),
            step("engine"),
            step("text_model"),
            step("vision_model"),
            step("embedding_model"),
            step("embedding_warmup"),
            step("persist"),
        ],
        "cloud" => vec![step("validate"), step("embedding_warmup"), step("persist")],
        other => {
            return Err(AppError::Validation(format!("Unknown mode: {other}")));
        }
    };

    // Single write lock: check-then-set atomically to prevent TOCTOU races.
    let job = {
        let mut lock = job_state().write().await;
        if let Some(ref existing) = *lock
            && (existing.status == JobStatus::InProgress || existing.status == JobStatus::Pending)
        {
            return Ok((axum::http::StatusCode::ACCEPTED, Json(existing.clone())));
        }
        let new_job = SetupJobState {
            job_id: format!("setup-{}", uuid::Uuid::new_v4().as_simple()),
            status: JobStatus::Pending,
            current_step: None,
            steps,
            error: None,
        };
        *lock = Some(new_job.clone());
        new_job
    };

    // Spawn the background job
    let state_clone = state.clone();
    tokio::spawn(async move {
        match body.mode.as_str() {
            "local" => run_local_setup(state_clone, body).await,
            "cloud" => run_cloud_setup(state_clone, body).await,
            _ => {} // Already validated above
        }
    });

    Ok((axum::http::StatusCode::ACCEPTED, Json(job)))
}

/// GET /api/onboarding/setup/{job_id} — poll for progress.
pub async fn get_setup_status(Path(job_id): Path<String>) -> Result<Json<SetupJobState>, AppError> {
    let lock = job_state().read().await;
    match &*lock {
        Some(job) if job.job_id == job_id => Ok(Json(job.clone())),
        _ => Err(AppError::NotFound(format!(
            "Setup job '{job_id}' not found"
        ))),
    }
}

/// DELETE /api/onboarding/setup/{job_id} — cancel.
pub async fn cancel_setup_job(Path(job_id): Path<String>) -> Result<Json<SetupJobState>, AppError> {
    let mut lock = job_state().write().await;
    match &mut *lock {
        Some(job) if job.job_id == job_id => {
            job.status = JobStatus::Canceled;
            Ok(Json(job.clone()))
        }
        _ => Err(AppError::NotFound(format!(
            "Setup job '{job_id}' not found"
        ))),
    }
}

fn step(id: &str) -> SetupStep {
    SetupStep {
        id: id.into(),
        status: StepStatus::Pending,
        message: None,
        progress: None,
    }
}

// ── Job runners ────────────────────────────────────────────────────────────

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

async fn run_local_setup(state: Arc<AppState>, body: SetupRequest) {
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
    if let Err(e) = std::fs::create_dir_all(&data_dir) {
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
    let required_bytes = match tier {
        "small" => 6_000_000_000_u64,
        "medium" => 11_000_000_000,
        _ => 15_000_000_000,
    };
    if let Some(available) = available_disk_space(&data_dir)
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

async fn run_cloud_setup(state: Arc<AppState>, body: SetupRequest) {
    {
        let mut lock = job_state().write().await;
        if let Some(ref mut job) = *lock {
            job.status = JobStatus::InProgress;
        }
    }

    let provider = body.provider.as_deref().unwrap_or("openai");

    // Step 1: Validate API key
    update_step("validate", StepStatus::InProgress, None).await;

    match provider {
        "openai" => {
            let api_key = match body.api_key.as_ref().filter(|k| !k.is_empty()) {
                Some(k) => k.clone(),
                None => {
                    update_step(
                        "validate",
                        StepStatus::Failed,
                        Some("API key is required".into()),
                    )
                    .await;
                    finish_job(
                        JobStatus::Failed,
                        Some("API key is required for OpenAI".into()),
                    )
                    .await;
                    return;
                }
            };

            // Test the API key by listing models
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
                    let status = resp.status();
                    let msg = format!("API key validation failed: HTTP {status}");
                    update_step("validate", StepStatus::Failed, Some(msg.clone())).await;
                    finish_job(JobStatus::Failed, Some(msg)).await;
                    return;
                }
                Err(e) => {
                    let msg = format!("Cannot reach OpenAI: {e}");
                    update_step("validate", StepStatus::Failed, Some(msg.clone())).await;
                    finish_job(JobStatus::Failed, Some(msg)).await;
                    return;
                }
            }

            // Step 2: Test embedding
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

            match test_openai_embedding(&state, &api_key, &model).await {
                Ok(_) => {
                    update_step(
                        "embedding_warmup",
                        StepStatus::Succeeded,
                        Some("Embedding endpoint working".into()),
                    )
                    .await;
                }
                Err(e) => {
                    warn!(error = %e, "setup.embedding_test_failed");
                    update_step(
                        "embedding_warmup",
                        StepStatus::Failed,
                        Some(format!("Embedding test failed: {e}")),
                    )
                    .await;
                    finish_job(
                        JobStatus::Failed,
                        Some(format!("Embedding test failed: {e}")),
                    )
                    .await;
                    return;
                }
            }

            // Step 3: Persist
            update_step("persist", StepStatus::InProgress, None).await;
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
        }
        "azure_openai" => {
            let api_key = match body.api_key.as_ref().filter(|k| !k.is_empty()) {
                Some(k) => k.clone(),
                None => {
                    update_step(
                        "validate",
                        StepStatus::Failed,
                        Some("API key is required".into()),
                    )
                    .await;
                    finish_job(JobStatus::Failed, Some("API key required".into())).await;
                    return;
                }
            };
            let endpoint = match body.endpoint.as_ref().filter(|e| !e.is_empty()) {
                Some(e) => e.clone(),
                None => {
                    update_step(
                        "validate",
                        StepStatus::Failed,
                        Some("Endpoint is required".into()),
                    )
                    .await;
                    finish_job(JobStatus::Failed, Some("Endpoint required".into())).await;
                    return;
                }
            };
            let deployment = match body.deployment.as_ref().filter(|d| !d.is_empty()) {
                Some(d) => d.clone(),
                None => {
                    update_step(
                        "validate",
                        StepStatus::Failed,
                        Some("Deployment name is required".into()),
                    )
                    .await;
                    finish_job(JobStatus::Failed, Some("Deployment required".into())).await;
                    return;
                }
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
                    // 400 is OK — means the endpoint is reachable and authenticated
                    update_step(
                        "validate",
                        StepStatus::Succeeded,
                        Some("Azure endpoint validated".into()),
                    )
                    .await;
                }
                Ok(resp) => {
                    let status = resp.status();
                    let msg = format!("Azure validation failed: HTTP {status}");
                    update_step("validate", StepStatus::Failed, Some(msg.clone())).await;
                    finish_job(JobStatus::Failed, Some(msg)).await;
                    return;
                }
                Err(e) => {
                    let msg = format!("Cannot reach Azure endpoint: {e}");
                    update_step("validate", StepStatus::Failed, Some(msg.clone())).await;
                    finish_job(JobStatus::Failed, Some(msg)).await;
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
            match test_azure_embedding(&state, &endpoint, &api_key, &deployment).await {
                Ok(_) => {
                    update_step(
                        "embedding_warmup",
                        StepStatus::Succeeded,
                        Some("Embedding working".into()),
                    )
                    .await;
                }
                Err(e) => {
                    update_step(
                        "embedding_warmup",
                        StepStatus::Failed,
                        Some(format!("Embedding failed: {e}")),
                    )
                    .await;
                    finish_job(JobStatus::Failed, Some(format!("Embedding failed: {e}"))).await;
                    return;
                }
            }

            update_step("persist", StepStatus::InProgress, None).await;
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
        }
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
            return;
        }
    }

    info!(provider, "setup.cloud_complete");
    finish_job(JobStatus::Succeeded, None).await;
}

async fn test_openai_embedding(
    state: &Arc<AppState>,
    api_key: &str,
    model: &str,
) -> Result<(), AppError> {
    let current = state.config();
    let mut candidate = (**current).clone();
    drop(current);

    candidate.llm.backend = LlmBackend::Openai;
    candidate.llm.openai_api_key = api_key.to_string();
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
    candidate.llm.azure_openai_api_key = api_key.to_string();
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
fn available_disk_space(path: &std::path::Path) -> Option<u64> {
    // fs4 or platform-specific APIs could be used, but a simple cross-platform
    // approach: use the unstable `Metadata` method or shell out to `df`.
    let output = std::process::Command::new("df")
        .arg("-k")
        .arg(path)
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    // df -k output: Filesystem 1K-blocks Used Available ...
    let line = stdout.lines().nth(1)?;
    let available_kb: u64 = line.split_whitespace().nth(3)?.parse().ok()?;
    Some(available_kb * 1024)
}
