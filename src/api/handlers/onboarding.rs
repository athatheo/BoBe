use std::sync::Arc;

use axum::Json;
use axum::extract::State;
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

#[derive(Debug, Serialize)]
pub struct WarmupEmbeddingResponse {
    pub ok: bool,
    pub message: String,
}

// ── Handlers ────────────────────────────────────────────────────────────────

/// GET /onboarding/status
///
/// Checks database health, LLM availability, and required models.
pub async fn onboarding_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<OnboardingStatusResponse>, AppError> {
    use crate::config::LlmBackend;

    let mut steps = std::collections::HashMap::new();

    // Check database
    let db_ok = sqlx::query("SELECT 1").fetch_one(&state.db).await.is_ok();

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
    let llm_configured = match cfg.llm.backend {
        LlmBackend::Ollama => true,
        LlmBackend::Openai => !cfg.llm.openai_api_key.is_empty(),
        LlmBackend::AzureOpenai => {
            !cfg.llm.azure_openai_api_key.is_empty()
                && !cfg.llm.azure_openai_endpoint.is_empty()
                && !cfg.llm.azure_openai_deployment.is_empty()
        }
        LlmBackend::LlamaCpp => true,
        LlmBackend::None => false,
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
                format!("LLM backend '{}' configured", cfg.llm.backend)
            } else {
                "No LLM backend configured".into()
            },
        },
    );

    let ollama_reachable = if matches!(cfg.llm.backend, LlmBackend::Ollama | LlmBackend::LlamaCpp) {
        let model_url = format!("{}/api/tags", cfg.ollama.url);
        reqwest::get(&model_url)
            .await
            .map(|resp| resp.status().is_success())
            .unwrap_or(false)
    } else {
        false
    };

    // Check Ollama models if backend is Ollama
    if cfg.llm.backend == LlmBackend::Ollama {
        let models_ok = ollama_reachable;

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

    // Check embedding readiness based on active backend.
    let embedding_ready = match cfg.llm.backend {
        LlmBackend::Openai => !cfg.llm.openai_api_key.is_empty(),
        LlmBackend::AzureOpenai => {
            !cfg.llm.azure_openai_api_key.is_empty()
                && !cfg.llm.azure_openai_endpoint.is_empty()
                && !cfg.llm.azure_openai_deployment.is_empty()
        }
        LlmBackend::Ollama | LlmBackend::LlamaCpp => ollama_reachable,
        LlmBackend::None => false,
    };

    let embedding_detail = match cfg.llm.backend {
        LlmBackend::Openai if embedding_ready => "OpenAI embedding configuration ready".to_string(),
        LlmBackend::Openai => "OpenAI embedding configuration incomplete".to_string(),
        LlmBackend::AzureOpenai if embedding_ready => {
            "Azure OpenAI embedding configuration ready".to_string()
        }
        LlmBackend::AzureOpenai => "Azure OpenAI embedding configuration incomplete".to_string(),
        LlmBackend::Ollama | LlmBackend::LlamaCpp if embedding_ready => {
            "Ollama embedding service reachable".to_string()
        }
        LlmBackend::Ollama | LlmBackend::LlamaCpp => {
            "Ollama is required for local embeddings".to_string()
        }
        LlmBackend::None => "No embedding backend configured".to_string(),
    };

    steps.insert(
        "embedding".into(),
        OnboardingStepStatus {
            status: if embedding_ready {
                "complete"
            } else {
                "incomplete"
            }
            .into(),
            detail: embedding_detail,
        },
    );

    let complete = steps.values().all(|s| s.status == "complete");
    let needs_onboarding = !llm_configured;

    Ok(Json(OnboardingStatusResponse {
        complete,
        needs_onboarding,
        steps,
    }))
}

/// POST /onboarding/complete
pub async fn mark_complete(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<MarkCompleteResponse>, AppError> {
    tracing::info!("onboarding.marked_complete");
    Ok(Json(MarkCompleteResponse { ok: true }))
}

/// POST /onboarding/warmup-embedding
///
/// Warms up the embedding model by generating a test embedding.
pub async fn warmup_embedding(
    State(state): State<Arc<AppState>>,
) -> Result<Json<WarmupEmbeddingResponse>, AppError> {
    tracing::info!("onboarding.warmup_embedding.start");
    match state.embedding_provider.embed("warmup").await {
        Ok(_) => {
            tracing::info!("onboarding.warmup_embedding.complete");
            Ok(Json(WarmupEmbeddingResponse {
                ok: true,
                message: "Embedding model ready".into(),
            }))
        }
        Err(e) => {
            tracing::error!(error = %e, "onboarding.warmup_embedding.failed");
            Ok(Json(WarmupEmbeddingResponse {
                ok: false,
                message: format!("Embedding warmup failed: {e}"),
            }))
        }
    }
}
