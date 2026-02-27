use std::sync::Arc;

use axum::Json;
use axum::extract::State;
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
    let llm_configured = match cfg.llm_backend {
        LlmBackend::Ollama => true,
        LlmBackend::Openai => !cfg.openai_api_key.is_empty(),
        LlmBackend::AzureOpenai => {
            !cfg.azure_openai_api_key.is_empty()
                && !cfg.azure_openai_endpoint.is_empty()
                && !cfg.azure_openai_deployment.is_empty()
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
                format!("LLM backend '{}' configured", cfg.llm_backend)
            } else {
                "No LLM backend configured".into()
            },
        },
    );

    let ollama_reachable = if matches!(cfg.llm_backend, LlmBackend::Ollama | LlmBackend::LlamaCpp) {
        let model_url = format!("{}/api/tags", cfg.ollama_url);
        reqwest::get(&model_url)
            .await
            .map(|resp| resp.status().is_success())
            .unwrap_or(false)
    } else {
        false
    };

    // Check Ollama models if backend is Ollama
    if cfg.llm_backend == LlmBackend::Ollama {
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
    let embedding_ready = match cfg.llm_backend {
        LlmBackend::Openai => !cfg.openai_api_key.is_empty(),
        LlmBackend::AzureOpenai => {
            !cfg.azure_openai_api_key.is_empty()
                && !cfg.azure_openai_endpoint.is_empty()
                && !cfg.azure_openai_deployment.is_empty()
        }
        LlmBackend::Ollama | LlmBackend::LlamaCpp => ollama_reachable,
        LlmBackend::None => false,
    };

    let embedding_detail = match cfg.llm_backend {
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

/// POST /onboarding/configure-llm
///
/// Validates and persists the LLM configuration via ConfigManager.
/// API keys and models are persisted through ConfigManager for restart safety.
pub async fn configure_llm(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ConfigureLlmRequest>,
) -> Result<Json<ConfigureLlmResponse>, AppError> {
    let cfg = state.config();
    let mode = body.mode.as_str();

    match mode {
        "ollama" => {
            let mut changes = std::collections::HashMap::new();
            changes.insert(
                "llm_backend".to_string(),
                serde_json::Value::String("ollama".into()),
            );
            if let Some(ref model) = body.model {
                changes.insert(
                    "ollama_model".to_string(),
                    serde_json::Value::String(model.clone()),
                );
            }
            state.config_manager.update(&changes);
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
            let mut changes = std::collections::HashMap::new();
            changes.insert(
                "llm_backend".to_string(),
                serde_json::Value::String("openai".into()),
            );
            if let Some(ref model) = body.model {
                changes.insert(
                    "openai_model".to_string(),
                    serde_json::Value::String(model.clone()),
                );
            }
            if let Some(ref key) = body.api_key {
                changes.insert(
                    "openai_api_key".to_string(),
                    serde_json::Value::String(key.clone()),
                );
            }
            state.config_manager.update(&changes);
            Ok(Json(ConfigureLlmResponse {
                ok: true,
                message: "Configured OpenAI".into(),
            }))
        }
        "azure_openai" => {
            let deployment = body.model.clone().filter(|m| !m.is_empty()).or_else(|| {
                if cfg.azure_openai_deployment.is_empty() {
                    None
                } else {
                    Some(cfg.azure_openai_deployment.clone())
                }
            });
            if body.api_key.as_ref().is_none_or(|k| k.is_empty())
                || body.endpoint.as_ref().is_none_or(|e| e.is_empty())
                || deployment.is_none()
            {
                return Ok(Json(ConfigureLlmResponse {
                    ok: false,
                    message: "API key, endpoint, and deployment required for Azure OpenAI".into(),
                }));
            }
            let mut changes = std::collections::HashMap::new();
            changes.insert(
                "llm_backend".to_string(),
                serde_json::Value::String("azure_openai".into()),
            );
            if let Some(ref endpoint) = body.endpoint {
                changes.insert(
                    "azure_openai_endpoint".to_string(),
                    serde_json::Value::String(endpoint.clone()),
                );
            }
            changes.insert(
                "azure_openai_deployment".to_string(),
                serde_json::Value::String(deployment.unwrap_or_default()),
            );
            if let Some(ref key) = body.api_key {
                changes.insert(
                    "azure_openai_api_key".to_string(),
                    serde_json::Value::String(key.clone()),
                );
            }
            state.config_manager.update(&changes);
            Ok(Json(ConfigureLlmResponse {
                ok: true,
                message: "Configured Azure OpenAI".into(),
            }))
        }
        "local" => {
            let mut changes = std::collections::HashMap::new();
            changes.insert(
                "llm_backend".to_string(),
                serde_json::Value::String("llamacpp".into()),
            );
            if let Some(ref url) = body.model {
                changes.insert(
                    "llama_url".to_string(),
                    serde_json::Value::String(url.clone()),
                );
            }
            state.config_manager.update(&changes);
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

/// POST /onboarding/pull-model
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
