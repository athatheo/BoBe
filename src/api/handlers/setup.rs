//! Setup handler — HTTP endpoints for the onboarding setup flow.
//!
//! DTOs and route handlers live here. The multi-step provisioning logic
//! is in `services::setup_service`.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::error::AppError;
use crate::services::setup_service::{
    job_state, run_cloud_setup, run_local_setup, tier_disk_estimate,
};

// ── Onboarding options ─────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct LocalTier {
    pub id: String,
    pub label: String,
    pub description: String,
    pub disk_estimate_bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct ModelChoice {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Serialize)]
pub struct CloudProvider {
    pub id: String,
    pub label: String,
    pub requires: Vec<String>,
    pub models: Vec<ModelChoice>,
}

#[derive(Debug, Serialize)]
pub struct OnboardingOptions {
    pub local_tiers: Vec<LocalTier>,
    pub cloud_providers: Vec<CloudProvider>,
}

// ── Cloud model definitions ─────────────────────────────────────────────
// Single source of truth for cloud model choices.
// The frontend reads these via GET /api/onboarding/options and
// displays labels directly — no client-side mapping needed.
// First model in each list is the default selection.
// To add/remove/rename models, only edit this list.

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
                disk_estimate_bytes: tier_disk_estimate("small"),
            },
            LocalTier {
                id: "medium".into(),
                label: "Medium (8B)".into(),
                description: "Balanced performance and quality.".into(),
                disk_estimate_bytes: tier_disk_estimate("medium"),
            },
            LocalTier {
                id: "large".into(),
                label: "Large (14B)".into(),
                description: "Best quality, requires more resources.".into(),
                disk_estimate_bytes: tier_disk_estimate("large"),
            },
        ],
        cloud_providers: vec![
            CloudProvider {
                id: "openai".into(),
                label: "OpenAI".into(),
                requires: vec!["api_key".into()],
                models: vec![
                    ModelChoice {
                        id: "gpt-5-mini".into(),
                        label: "GPT-5 Mini".into(),
                    },
                    ModelChoice {
                        id: "gpt-5-nano".into(),
                        label: "GPT-5 Nano".into(),
                    },
                    ModelChoice {
                        id: "gpt-5.2".into(),
                        label: "GPT-5.2".into(),
                    },
                ],
            },
            CloudProvider {
                id: "azure_openai".into(),
                label: "Azure OpenAI".into(),
                requires: vec!["api_key".into(), "endpoint".into(), "deployment".into()],
                models: vec![],
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
