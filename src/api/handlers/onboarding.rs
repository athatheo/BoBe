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

    // Check LLM/backend connectivity and setup readiness.
    // Snapshot config values before async calls so we don't hold the Arc guard across awaits.
    let cfg = state.config();
    let backend = cfg.llm.backend;
    let ollama_url = cfg.ollama.url.clone();
    let llama_url = cfg.llm.llama_url.clone();
    let openai_api_key = cfg.llm.openai_api_key.clone();
    let azure_endpoint = cfg.llm.azure_openai_endpoint.clone();
    let azure_api_key = cfg.llm.azure_openai_api_key.clone();
    let azure_deployment = cfg.llm.azure_openai_deployment.clone();
    let setup_completed = cfg.setup_completed;
    drop(cfg);

    let client = &state.http_client;
    let (llm_ready, llm_detail, local_models_ready, local_models_detail) = match backend {
        LlmBackend::Ollama => {
            let url = format!("{}/api/tags", ollama_url.trim_end_matches('/'));
            match client.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => (
                    true,
                    "Ollama backend reachable".to_string(),
                    true,
                    "Ollama running and accessible".to_string(),
                ),
                Ok(resp) => (
                    false,
                    format!("Ollama unreachable: HTTP {}", resp.status()),
                    false,
                    "Cannot reach Ollama".to_string(),
                ),
                Err(e) => (
                    false,
                    format!("Cannot reach Ollama: {e}"),
                    false,
                    "Cannot reach Ollama".to_string(),
                ),
            }
        }
        LlmBackend::LlamaCpp => {
            let url = format!("{}/v1/models", llama_url.trim_end_matches('/'));
            match client.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => (
                    true,
                    "llama.cpp backend reachable".to_string(),
                    true,
                    "n/a".to_string(),
                ),
                Ok(resp) => (
                    false,
                    format!("llama.cpp unreachable: HTTP {}", resp.status()),
                    true,
                    "n/a".to_string(),
                ),
                Err(e) => (
                    false,
                    format!("Cannot reach llama.cpp backend: {e}"),
                    true,
                    "n/a".to_string(),
                ),
            }
        }
        LlmBackend::Openai => {
            if openai_api_key.is_empty() {
                (
                    false,
                    "OpenAI API key missing".to_string(),
                    true,
                    "n/a".to_string(),
                )
            } else {
                match client
                    .get("https://api.openai.com/v1/models")
                    .header("Authorization", format!("Bearer {openai_api_key}"))
                    .send()
                    .await
                {
                    Ok(resp) if resp.status().is_success() => (
                        true,
                        "OpenAI backend validated".to_string(),
                        true,
                        "n/a".to_string(),
                    ),
                    Ok(resp) => (
                        false,
                        format!("OpenAI validation failed: HTTP {}", resp.status()),
                        true,
                        "n/a".to_string(),
                    ),
                    Err(e) => (
                        false,
                        format!("Cannot reach OpenAI: {e}"),
                        true,
                        "n/a".to_string(),
                    ),
                }
            }
        }
        LlmBackend::AzureOpenai => {
            if azure_endpoint.is_empty() || azure_api_key.is_empty() || azure_deployment.is_empty()
            {
                (
                    false,
                    "Azure OpenAI endpoint/API key/deployment missing".to_string(),
                    true,
                    "n/a".to_string(),
                )
            } else {
                let test_url = format!(
                    "{}/openai/deployments/{}/chat/completions?api-version=2024-02-15-preview",
                    azure_endpoint.trim_end_matches('/'),
                    azure_deployment
                );
                match client
                    .post(&test_url)
                    .header("api-key", &azure_api_key)
                    .json(&serde_json::json!({"messages": [{"role": "user", "content": "test"}], "max_tokens": 1}))
                    .send()
                    .await
                {
                    Ok(resp) if resp.status().is_success() || resp.status().as_u16() == 400 => (
                        true,
                        "Azure OpenAI backend validated".to_string(),
                        true,
                        "n/a".to_string(),
                    ),
                    Ok(resp) => (
                        false,
                        format!("Azure validation failed: HTTP {}", resp.status()),
                        true,
                        "n/a".to_string(),
                    ),
                    Err(e) => (
                        false,
                        format!("Cannot reach Azure endpoint: {e}"),
                        true,
                        "n/a".to_string(),
                    ),
                }
            }
        }
        LlmBackend::None => (
            false,
            "No LLM backend configured".to_string(),
            true,
            "n/a".to_string(),
        ),
    };

    steps.insert(
        "llm".into(),
        OnboardingStepStatus {
            status: if llm_ready { "complete" } else { "incomplete" }.into(),
            detail: llm_detail,
        },
    );

    if backend == LlmBackend::Ollama {
        steps.insert(
            "models".into(),
            OnboardingStepStatus {
                status: if local_models_ready {
                    "complete"
                } else {
                    "incomplete"
                }
                .into(),
                detail: local_models_detail,
            },
        );
    }

    let embedding_ready = llm_ready;
    let embedding_detail = if embedding_ready {
        "Embedding backend reachable".to_string()
    } else {
        "Embedding backend not ready".to_string()
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

    let setup_ready = llm_ready && local_models_ready && embedding_ready;
    let complete = db_ok && setup_ready;
    // Once setup has been completed once, don't re-trigger the wizard just
    // because the LLM backend is temporarily unreachable (e.g. no internet).
    let needs_onboarding = !setup_ready && !setup_completed;

    Ok(Json(OnboardingStatusResponse {
        complete,
        needs_onboarding,
        steps,
    }))
}

/// POST /onboarding/mark-complete
///
/// Persists `setup_completed = true` so future launches don't re-trigger the
/// onboarding wizard when the LLM backend is temporarily unreachable.
pub async fn mark_complete(
    State(state): State<Arc<AppState>>,
) -> Result<Json<MarkCompleteResponse>, AppError> {
    let mut changes = std::collections::HashMap::new();
    changes.insert("setup_completed".to_string(), serde_json::Value::Bool(true));
    state.config_manager.update(&changes);
    tracing::info!("onboarding.marked_complete");
    Ok(Json(MarkCompleteResponse { ok: true }))
}
