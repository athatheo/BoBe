use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::app_state::AppState;
use crate::error::AppError;

// ── Schemas ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct OnboardingStepStatus {
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Serialize)]
pub struct OnboardingStatusResponse {
    pub complete: bool,
    pub needs_onboarding: bool,
    pub steps: std::collections::HashMap<String, OnboardingStepStatus>,
}

#[derive(Debug, Serialize)]
pub struct MarkCompleteResponse {
    pub ok: bool,
}

// ── Handlers ────────────────────────────────────────────────────────────────

/// GET /api/onboarding/status
///
/// Checks database health, LLM availability, and required models.
pub async fn onboarding_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<OnboardingStatusResponse>, AppError> {
    let mut steps = std::collections::HashMap::new();

    // Check database
    let db_ok = sqlx::query("SELECT 1")
        .fetch_one(&state.db)
        .await
        .is_ok();

    steps.insert(
        "database".into(),
        OnboardingStepStatus {
            status: if db_ok { "complete" } else { "error" }.into(),
            detail: if db_ok {
                "Database connected".into()
            } else {
                "Database connection failed".into()
            },
        },
    );

    // Check LLM backend configuration
    let cfg = state.config();
    let llm_configured = match cfg.llm_backend.as_str() {
        "ollama" => true,
        "openai" => !cfg.openai_api_key.is_empty(),
        "azure_openai" => {
            !cfg.azure_openai_api_key.is_empty() && !cfg.azure_openai_endpoint.is_empty()
        }
        "local" => true,
        _ => false,
    };

    steps.insert(
        "llm".into(),
        OnboardingStepStatus {
            status: if llm_configured {
                "complete"
            } else {
                "incomplete"
            }
            .into(),
            detail: if llm_configured {
                format!("LLM backend '{}' configured", cfg.llm_backend)
            } else {
                "No LLM backend configured".into()
            },
        },
    );

    // Check Ollama models if backend is Ollama
    if cfg.llm_backend == "ollama" {
        let model_url = format!("{}/api/tags", cfg.ollama_url);
        let models_ok = reqwest::get(&model_url).await.is_ok();

        steps.insert(
            "models".into(),
            OnboardingStepStatus {
                status: if models_ok { "complete" } else { "incomplete" }.into(),
                detail: if models_ok {
                    "Ollama running and accessible".into()
                } else {
                    "Cannot reach Ollama".into()
                },
            },
        );
    }

    let complete = steps.values().all(|s| s.status == "complete");
    let needs_onboarding = !llm_configured;

    Ok(Json(OnboardingStatusResponse {
        complete,
        needs_onboarding,
        steps,
    }))
}

/// POST /api/onboarding/complete
pub async fn mark_complete(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<MarkCompleteResponse>, AppError> {
    tracing::info!("onboarding.marked_complete");
    Ok(Json(MarkCompleteResponse { ok: true }))
}
