use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Deserialize)]
pub struct ConfigureLlmRequest {
    pub mode: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub endpoint: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ConfigureLlmResponse {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct PullModelRequest {
    pub model: String,
}

#[derive(Debug, Serialize)]
pub struct PullModelResponse {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct WarmupEmbeddingResponse {
    pub ok: bool,
    pub message: String,
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
    let llm_configured = match cfg.llm_backend {
        crate::config::LlmBackend::Ollama => true,
        crate::config::LlmBackend::Openai => !cfg.openai_api_key.is_empty(),
        crate::config::LlmBackend::AzureOpenai => {
            !cfg.azure_openai_api_key.is_empty() && !cfg.azure_openai_endpoint.is_empty()
        }
        crate::config::LlmBackend::LlamaCpp => true,
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
    if cfg.llm_backend == crate::config::LlmBackend::Ollama {
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

/// POST /api/onboarding/configure-llm
///
/// Validates and persists the LLM configuration to ~/.bobe/.env.
/// API keys are set as env vars for the running process but NOT written to disk.
pub async fn configure_llm(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ConfigureLlmRequest>,
) -> Result<Json<ConfigureLlmResponse>, AppError> {
    use crate::composition::config_persistence::persist_config;
    use std::collections::BTreeMap;

    let cfg = state.config();
    let mode = body.mode.as_str();

    match mode {
        "ollama" => {
            let mut changes = BTreeMap::new();
            changes.insert("BOBE_LLM_BACKEND".into(), "ollama".into());
            if let Some(ref model) = body.model {
                changes.insert("BOBE_OLLAMA_MODEL".into(), model.clone());
            }
            persist_config(&changes);
            let model = body.model.unwrap_or_else(|| cfg.ollama_model.clone());
            Ok(Json(ConfigureLlmResponse {
                ok: true,
                message: format!("Configured Ollama with model {model}"),
            }))
        }
        "openai" => {
            if body.api_key.as_ref().is_none_or(|k| k.is_empty()) {
                return Ok(Json(ConfigureLlmResponse {
                    ok: false,
                    message: "API key required for OpenAI".into(),
                }));
            }
            let mut changes = BTreeMap::new();
            changes.insert("BOBE_LLM_BACKEND".into(), "openai".into());
            if let Some(ref model) = body.model {
                changes.insert("BOBE_OPENAI_MODEL".into(), model.clone());
            }
            persist_config(&changes);
            // API key: env var only (never persisted to .env for security).
            // SAFETY: set_var in multi-threaded context is technically UB per Rust spec,
            // but macOS/glibc setenv is thread-safe. Acceptable for infrequent config ops.
            if let Some(ref key) = body.api_key {
                unsafe { std::env::set_var("BOBE_OPENAI_API_KEY", key); }
            }
            Ok(Json(ConfigureLlmResponse {
                ok: true,
                message: "Configured OpenAI".into(),
            }))
        }
        "azure_openai" => {
            if body.api_key.as_ref().is_none_or(|k| k.is_empty())
                || body.endpoint.as_ref().is_none_or(|e| e.is_empty())
            {
                return Ok(Json(ConfigureLlmResponse {
                    ok: false,
                    message: "API key and endpoint required for Azure OpenAI".into(),
                }));
            }
            let mut changes = BTreeMap::new();
            changes.insert("BOBE_LLM_BACKEND".into(), "azure_openai".into());
            if let Some(ref endpoint) = body.endpoint {
                changes.insert("BOBE_AZURE_OPENAI_ENDPOINT".into(), endpoint.clone());
            }
            if let Some(ref model) = body.model {
                changes.insert("BOBE_AZURE_OPENAI_DEPLOYMENT".into(), model.clone());
            }
            persist_config(&changes);
            if let Some(ref key) = body.api_key {
                unsafe { std::env::set_var("BOBE_AZURE_OPENAI_API_KEY", key); }
            }
            Ok(Json(ConfigureLlmResponse {
                ok: true,
                message: "Configured Azure OpenAI".into(),
            }))
        }
        "local" => {
            let mut changes = BTreeMap::new();
            changes.insert("BOBE_LLM_BACKEND".into(), "local".into());
            if let Some(ref url) = body.model {
                changes.insert("BOBE_LLAMA_URL".into(), url.clone());
            }
            persist_config(&changes);
            Ok(Json(ConfigureLlmResponse {
                ok: true,
                message: "Configured local llama.cpp".into(),
            }))
        }
        other => Ok(Json(ConfigureLlmResponse {
            ok: false,
            message: format!("Unknown mode: {other}"),
        })),
    }
}

/// POST /api/onboarding/pull-model
///
/// Pulls an Ollama model by delegating to the OllamaManager.
pub async fn pull_model(
    State(state): State<Arc<AppState>>,
    Json(body): Json<PullModelRequest>,
) -> Result<Json<PullModelResponse>, AppError> {
    tracing::info!(model = %body.model, "onboarding.pull_model.start");
    state.ollama_manager.pull_model(&body.model).await?;
    tracing::info!(model = %body.model, "onboarding.pull_model.complete");
    Ok(Json(PullModelResponse {
        ok: true,
        message: format!("Model '{}' pulled successfully", body.model),
    }))
}

/// POST /api/onboarding/warmup-embedding
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
