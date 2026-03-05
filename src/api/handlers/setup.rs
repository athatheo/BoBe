//! Setup handler — onboarding setup flow. Provisioning logic in `services::setup_service`.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::error::AppError;
use crate::i18n::{FALLBACK_LOCALE, t, t_vars};
use crate::services::setup_service::{
    job_state, run_cloud_setup, run_local_setup, tier_disk_estimate,
};

#[derive(Debug, Serialize)]
pub(crate) struct LocalTier {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) description: String,
    pub(crate) disk_estimate_bytes: u64,
}

#[derive(Debug, Serialize)]
pub(crate) struct ModelChoice {
    pub(crate) id: String,
    pub(crate) label: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct CloudProvider {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) requires: Vec<String>,
    pub(crate) models: Vec<ModelChoice>,
}

#[derive(Debug, Serialize)]
pub(crate) struct OnboardingOptions {
    pub(crate) local_tiers: Vec<LocalTier>,
    pub(crate) cloud_providers: Vec<CloudProvider>,
}

pub(crate) async fn get_options(
    State(state): State<Arc<AppState>>,
) -> Result<Json<OnboardingOptions>, AppError> {
    let cfg = state.config();
    let locale = cfg.effective_locale();
    let locale = if locale.trim().is_empty() {
        FALLBACK_LOCALE.to_owned()
    } else {
        locale
    };

    Ok(Json(OnboardingOptions {
        local_tiers: vec![
            LocalTier {
                id: "small".into(),
                label: t(&locale, "onboarding-local-tier-small-label"),
                description: t(&locale, "onboarding-local-tier-small-description"),
                disk_estimate_bytes: tier_disk_estimate("small"),
            },
            LocalTier {
                id: "medium".into(),
                label: t(&locale, "onboarding-local-tier-medium-label"),
                description: t(&locale, "onboarding-local-tier-medium-description"),
                disk_estimate_bytes: tier_disk_estimate("medium"),
            },
            LocalTier {
                id: "large".into(),
                label: t(&locale, "onboarding-local-tier-large-label"),
                description: t(&locale, "onboarding-local-tier-large-description"),
                disk_estimate_bytes: tier_disk_estimate("large"),
            },
        ],
        cloud_providers: vec![
            CloudProvider {
                id: "openai".into(),
                label: t(&locale, "onboarding-cloud-provider-openai-label"),
                requires: vec!["api_key".into()],
                models: vec![
                    ModelChoice {
                        id: "gpt-5-mini".into(),
                        label: t(
                            &locale,
                            "onboarding-cloud-provider-openai-model-gpt-5-mini-label",
                        ),
                    },
                    ModelChoice {
                        id: "gpt-5-nano".into(),
                        label: t(
                            &locale,
                            "onboarding-cloud-provider-openai-model-gpt-5-nano-label",
                        ),
                    },
                    ModelChoice {
                        id: "gpt-5.2".into(),
                        label: t(
                            &locale,
                            "onboarding-cloud-provider-openai-model-gpt-5-2-label",
                        ),
                    },
                ],
            },
            CloudProvider {
                id: "azure_openai".into(),
                label: t(&locale, "onboarding-cloud-provider-azure-openai-label"),
                requires: vec!["api_key".into(), "endpoint".into(), "deployment".into()],
                models: vec![],
            },
        ],
    }))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StepStatus {
    Pending,
    InProgress,
    Succeeded,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct StepProgress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) percent: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) current_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) total_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SetupStep {
    pub(crate) id: String,
    pub(crate) status: StepStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) progress: Option<StepProgress>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum JobStatus {
    Pending,
    InProgress,
    Succeeded,
    Failed,
    Canceled,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SetupJobState {
    pub(crate) job_id: String,
    pub(crate) status: JobStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) current_step: Option<String>,
    pub(crate) steps: Vec<SetupStep>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SetupRequest {
    pub(crate) mode: String,
    #[serde(default)]
    pub(crate) tier: Option<String>,
    #[serde(default)]
    pub(crate) provider: Option<String>,
    #[serde(default)]
    pub(crate) api_key: Option<String>,
    #[serde(default)]
    pub(crate) model: Option<String>,
    #[serde(default)]
    pub(crate) endpoint: Option<String>,
    #[serde(default)]
    pub(crate) deployment: Option<String>,
}

/// 202 Accepted; spawns background setup job.
pub(crate) async fn create_setup_job(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SetupRequest>,
) -> Result<(axum::http::StatusCode, Json<SetupJobState>), AppError> {
    let locale = effective_locale(&state);

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
            return Err(AppError::Validation(t_vars(
                &locale,
                "setup-error-unknown-mode",
                &[("mode", other.to_owned())],
            )));
        }
    };

    // Atomic check-then-set to prevent TOCTOU races.
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

    let state_clone = Arc::clone(&state);
    tokio::spawn(async move {
        match body.mode.as_str() {
            "local" => run_local_setup(state_clone, body).await,
            "cloud" => run_cloud_setup(state_clone, body).await,
            _ => {} // Already validated above
        }
    });

    Ok((axum::http::StatusCode::ACCEPTED, Json(job)))
}

pub(crate) async fn get_setup_status(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<String>,
) -> Result<Json<SetupJobState>, AppError> {
    let locale = effective_locale(&state);
    let lock = job_state().read().await;
    match &*lock {
        Some(job) if job.job_id == job_id => Ok(Json(job.clone())),
        _ => Err(AppError::NotFound(t_vars(
            &locale,
            "setup-error-job-not-found",
            &[("job_id", job_id)],
        ))),
    }
}

pub(crate) async fn cancel_setup_job(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<String>,
) -> Result<Json<SetupJobState>, AppError> {
    let locale = effective_locale(&state);
    let mut lock = job_state().write().await;
    match &mut *lock {
        Some(job) if job.job_id == job_id => {
            job.status = JobStatus::Canceled;
            Ok(Json(job.clone()))
        }
        _ => Err(AppError::NotFound(t_vars(
            &locale,
            "setup-error-job-not-found",
            &[("job_id", job_id)],
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

fn effective_locale(state: &Arc<AppState>) -> String {
    let locale = state.config().effective_locale();
    if locale.trim().is_empty() {
        FALLBACK_LOCALE.to_owned()
    } else {
        locale
    }
}
