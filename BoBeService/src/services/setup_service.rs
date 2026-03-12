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
use crate::i18n::{t, t_vars};
use crate::llm::factory::LlmProviderFactory;
use crate::services::ollama_runtime_service::OllamaRuntimeService;

// ── Shared job state ────────────────────────────────────────────────────────

pub(crate) type SharedJobState = Arc<RwLock<Option<SetupJobState>>>;

static SETUP_JOB: std::sync::OnceLock<SharedJobState> = std::sync::OnceLock::new();

pub(crate) fn job_state() -> &'static SharedJobState {
    SETUP_JOB.get_or_init(|| Arc::new(RwLock::new(None)))
}

// ── Model tier mapping ──────────────────────────────────────────────────────

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

#[derive(Debug, Clone)]
struct SetupLocalizer {
    locale: String,
}

impl SetupLocalizer {
    fn from_state(state: &Arc<AppState>) -> Self {
        let cfg = state.config();
        Self {
            locale: cfg.effective_locale(),
        }
    }

    fn text(&self, key: &'static str) -> String {
        t(&self.locale, key)
    }

    fn vars(&self, key: &'static str, args: &[(&str, String)]) -> String {
        t_vars(&self.locale, key, args)
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
    let localizer = SetupLocalizer::from_state(&state);

    let tier = body.tier.as_deref().unwrap_or("large");
    let models = tier_models(tier);

    update_step("validate", StepStatus::InProgress, None).await;
    let data_dir = state.config().resolved_data_dir();
    if let Err(e) = tokio::fs::create_dir_all(&data_dir).await {
        let msg = localizer.vars(
            "setup-error-create-data-directory",
            &[("error", e.to_string())],
        );
        update_step("validate", StepStatus::Failed, Some(msg.clone())).await;
        finish_job(JobStatus::Failed, Some(msg)).await;
        return;
    }

    let required_bytes = tier_disk_estimate(tier);
    if let Some(available) = available_disk_space(&data_dir).await
        && available < required_bytes
    {
        let needed_gb = required_bytes / 1_000_000_000;
        let avail_gb = available / 1_000_000_000;
        let msg = localizer.vars(
            "setup-error-not-enough-disk-space",
            &[
                ("needed_gb", needed_gb.to_string()),
                ("available_gb", avail_gb.to_string()),
            ],
        );
        update_step("validate", StepStatus::Failed, Some(msg.clone())).await;
        finish_job(JobStatus::Failed, Some(msg)).await;
        return;
    }

    update_step(
        "validate",
        StepStatus::Succeeded,
        Some(localizer.text("setup-step-validate-data-directory-ready")),
    )
    .await;

    if is_canceled().await {
        finish_job(JobStatus::Canceled, None).await;
        return;
    }

    let ollama_runtime = OllamaRuntimeService::from(&state);

    update_step("engine", StepStatus::InProgress, None).await;
    let (progress_tx, mut progress_rx) =
        tokio::sync::watch::channel(crate::binary_manager::DownloadProgress {
            current_bytes: 0,
            total_bytes: None,
            percent: None,
        });

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

    let binary_path = match ollama_runtime
        .prepare_managed_local_runtime(&progress_tx)
        .await
    {
        Ok(path) => {
            update_step(
                "engine",
                StepStatus::Succeeded,
                Some(localizer.vars(
                    "setup-step-engine-ollama-at",
                    &[("path", path.display().to_string())],
                )),
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
    drop(progress_relay.await);

    if is_canceled().await {
        finish_job(JobStatus::Canceled, None).await;
        return;
    }

    update_step(
        "text_model",
        StepStatus::InProgress,
        Some(localizer.vars(
            "setup-step-model-pulling",
            &[("model", models.text.to_owned())],
        )),
    )
    .await;
    match pull_model(&state, models.text).await {
        Ok(()) => {
            update_step(
                "text_model",
                StepStatus::Succeeded,
                Some(localizer.vars(
                    "setup-step-model-ready",
                    &[("model", models.text.to_owned())],
                )),
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

    let embedding_model = "all-minilm";
    update_step(
        "embedding_model",
        StepStatus::InProgress,
        Some(localizer.vars(
            "setup-step-model-pulling",
            &[("model", embedding_model.to_owned())],
        )),
    )
    .await;
    match pull_model(&state, embedding_model).await {
        Ok(()) => {
            update_step(
                "embedding_model",
                StepStatus::Succeeded,
                Some(localizer.vars(
                    "setup-step-model-ready",
                    &[("model", embedding_model.to_owned())],
                )),
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

    update_step(
        "embedding_warmup",
        StepStatus::InProgress,
        Some(localizer.text("setup-step-embedding-loading")),
    )
    .await;
    match test_local_embedding(&state).await {
        Ok(()) => {
            update_step(
                "embedding_warmup",
                StepStatus::Succeeded,
                Some(localizer.text("setup-step-embedding-loaded")),
            )
            .await;
        }
        Err(e) => {
            warn!(error = %e, "setup.embedding_warmup_failed");
            update_step(
                "embedding_warmup",
                StepStatus::Failed,
                Some(localizer.vars(
                    "setup-step-embedding-warmup-failed-non-fatal",
                    &[("error", e.to_string())],
                )),
            )
            .await;
        }
    }

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
        serde_json::Value::String("none".into()),
    );
    changes.insert(
        "vision.ollama_model".to_string(),
        serde_json::Value::String(models.vision.into()),
    );
    changes.insert(
        "capture.enabled".to_string(),
        serde_json::Value::Bool(false),
    );

    if !persist_config(&state, changes, &localizer).await {
        return;
    }

    info!("setup.local_complete");
    finish_job(JobStatus::Succeeded, None).await;
}

pub(crate) async fn run_local_vision_setup(state: Arc<AppState>, body: SetupRequest) {
    {
        let mut lock = job_state().write().await;
        if let Some(ref mut job) = *lock {
            job.status = JobStatus::InProgress;
        }
    }

    let localizer = SetupLocalizer::from_state(&state);
    let tier = body.tier.as_deref().unwrap_or("large");
    let models = tier_models(tier);
    let ollama_runtime = OllamaRuntimeService::from(&state);

    update_step(
        "vision_model",
        StepStatus::InProgress,
        Some(localizer.vars(
            "setup-step-model-pulling",
            &[("model", models.vision.to_owned())],
        )),
    )
    .await;
    match ollama_runtime
        .ensure_model_ready(models.vision, || {
            SETUP_JOB
                .get()
                .and_then(|s| s.try_read().ok())
                .and_then(|lock| lock.as_ref().map(|j| j.status == JobStatus::Canceled))
                .unwrap_or(false)
        })
        .await
    {
        Ok(()) => {
            update_step(
                "vision_model",
                StepStatus::Succeeded,
                Some(localizer.vars(
                    "setup-step-model-ready",
                    &[("model", models.vision.to_owned())],
                )),
            )
            .await;
        }
        Err(e) => {
            update_step("vision_model", StepStatus::Failed, Some(e.to_string())).await;
            finish_job(JobStatus::Failed, Some(e.to_string())).await;
            return;
        }
    }

    if is_canceled().await {
        finish_job(JobStatus::Canceled, None).await;
        return;
    }

    let mut changes = HashMap::new();
    changes.insert(
        "vision.backend".to_string(),
        serde_json::Value::String("ollama".into()),
    );
    changes.insert(
        "vision.ollama_model".to_string(),
        serde_json::Value::String(models.vision.into()),
    );
    changes.insert("capture.enabled".to_string(), serde_json::Value::Bool(true));

    if !persist_config(&state, changes, &localizer).await {
        return;
    }

    info!("setup.local_vision_complete");
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
    let localizer = SetupLocalizer::from_state(&state);
    update_step("validate", StepStatus::InProgress, None).await;

    match provider {
        "openai" => run_openai_setup(&state, &body).await,
        "azure_openai" => run_azure_setup(&state, &body).await,
        other => {
            let msg = localizer.vars(
                "setup-error-unknown-provider",
                &[("provider", other.to_owned())],
            );
            update_step("validate", StepStatus::Failed, Some(msg.clone())).await;
            finish_job(JobStatus::Failed, Some(msg)).await;
        }
    }
}

async fn run_openai_setup(state: &Arc<AppState>, body: &SetupRequest) {
    let localizer = SetupLocalizer::from_state(state);
    let Some(api_key) = body
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .map(str::to_owned)
    else {
        let msg = localizer.text("setup-openai-error-api-key-required");
        fail_step("validate", msg.clone(), msg).await;
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
                Some(localizer.text("setup-openai-validation-api-key-valid")),
            )
            .await;
        }
        Ok(resp) => {
            let msg = localizer.vars(
                "setup-openai-error-validation-http",
                &[("status", resp.status().to_string())],
            );
            fail_step("validate", msg.clone(), msg).await;
            return;
        }
        Err(e) => {
            let msg = if e.is_builder() {
                localizer.text("setup-openai-error-invalid-api-key-format")
            } else {
                localizer.vars(
                    "setup-openai-error-cannot-reach",
                    &[("error", e.to_string())],
                )
            };
            fail_step("validate", msg.clone(), msg).await;
            return;
        }
    }

    update_step(
        "embedding_warmup",
        StepStatus::InProgress,
        Some(localizer.text("setup-openai-embedding-testing")),
    )
    .await;

    let model = body
        .model
        .as_deref()
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .unwrap_or("gpt-5-mini")
        .to_string();

    match test_openai_embedding(state, &api_key, &model).await {
        Ok(()) => {
            update_step(
                "embedding_warmup",
                StepStatus::Succeeded,
                Some(localizer.text("setup-openai-embedding-working")),
            )
            .await;
        }
        Err(e) => {
            warn!(error = %e, "setup.embedding_test_failed");
            let msg = localizer.vars("setup-openai-embedding-failed", &[("error", e.to_string())]);
            fail_step("embedding_warmup", msg.clone(), msg).await;
            return;
        }
    }

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
    if persist_config(state, changes, &localizer).await {
        info!("setup.openai_complete");
        finish_job(JobStatus::Succeeded, None).await;
    }
}

async fn run_azure_setup(state: &Arc<AppState>, body: &SetupRequest) {
    let localizer = SetupLocalizer::from_state(state);
    let Some(api_key) = body
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .map(str::to_owned)
    else {
        let msg = localizer.text("setup-azure-error-api-key-required");
        fail_step("validate", msg.clone(), msg).await;
        return;
    };
    let Some(endpoint) = body
        .endpoint
        .as_deref()
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .map(str::to_owned)
    else {
        let msg = localizer.text("setup-azure-error-endpoint-required");
        fail_step("validate", msg.clone(), msg).await;
        return;
    };
    let Some(deployment) = body
        .deployment
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .map(str::to_owned)
    else {
        let msg = localizer.text("setup-azure-error-deployment-required");
        fail_step("validate", msg.clone(), msg).await;
        return;
    };

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
                Some(localizer.text("setup-azure-validation-endpoint-validated")),
            )
            .await;
        }
        Ok(resp) => {
            let msg = localizer.vars(
                "setup-azure-error-validation-http",
                &[("status", resp.status().to_string())],
            );
            fail_step("validate", msg.clone(), msg).await;
            return;
        }
        Err(e) => {
            let msg = if e.is_builder() {
                localizer.text("setup-azure-error-invalid-value-format")
            } else {
                localizer.vars("setup-azure-error-cannot-reach", &[("error", e.to_string())])
            };
            fail_step("validate", msg.clone(), msg).await;
            return;
        }
    }

    update_step(
        "embedding_warmup",
        StepStatus::InProgress,
        Some(localizer.text("setup-azure-embedding-testing")),
    )
    .await;
    match test_azure_embedding(state, &endpoint, &api_key, &deployment).await {
        Ok(()) => {
            update_step(
                "embedding_warmup",
                StepStatus::Succeeded,
                Some(localizer.text("setup-azure-embedding-working")),
            )
            .await;
        }
        Err(e) => {
            let msg = localizer.vars("setup-azure-embedding-failed", &[("error", e.to_string())]);
            fail_step("embedding_warmup", msg.clone(), msg).await;
            return;
        }
    }

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
    if persist_config(state, changes, &localizer).await {
        info!("setup.azure_complete");
        finish_job(JobStatus::Succeeded, None).await;
    }
}

async fn fail_step(step_id: &str, step_msg: String, job_msg: String) {
    update_step(step_id, StepStatus::Failed, Some(step_msg)).await;
    finish_job(JobStatus::Failed, Some(job_msg)).await;
}

async fn persist_config(
    state: &Arc<AppState>,
    changes: HashMap<String, serde_json::Value>,
    localizer: &SetupLocalizer,
) -> bool {
    update_step("persist", StepStatus::InProgress, None).await;
    let update = state.config_manager.update(&changes);
    if update.persist_failed {
        let msg = localizer.text("setup-error-persist-failed");
        fail_step("persist", msg.clone(), msg).await;
        return false;
    }
    update_step(
        "persist",
        StepStatus::Succeeded,
        Some(localizer.text("setup-step-persist-saved")),
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

/// Warmup-test the local Ollama embedding provider against a candidate config
/// that has `llm.backend = ollama`.  Note: embeddings use `embedding.model`
/// (default `BAAI/bge-small-en-v1.5`), not the text LLM model.
async fn test_local_embedding(state: &Arc<AppState>) -> Result<(), AppError> {
    let current = state.config();
    let mut candidate = (**current).clone();
    drop(current);

    candidate.llm.backend = LlmBackend::Ollama;

    let factory = LlmProviderFactory::new(
        state.http_client.clone(),
        Arc::new(ArcSwap::from_pointee(candidate)),
    );
    let embedding = factory.create_embedding()?;
    embedding.embed("warmup").await.map(|_| ())
}

async fn pull_model(state: &Arc<AppState>, model: &str) -> Result<(), AppError> {
    OllamaRuntimeService::from(state)
        .ensure_model_ready(model, || {
            SETUP_JOB
                .get()
                .and_then(|s| s.try_read().ok())
                .and_then(|lock| lock.as_ref().map(|j| j.status == JobStatus::Canceled))
                .unwrap_or(false)
        })
        .await
}

async fn available_disk_space(path: &std::path::Path) -> Option<u64> {
    let output = tokio::process::Command::new("df")
        .args(["-Pk"]) // POSIX format, 1K blocks
        .arg(path)
        .output()
        .await
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().nth(1)?;
    let available_kb: u64 = line.split_whitespace().nth(3)?.parse().ok()?;
    Some(available_kb * 1024)
}
