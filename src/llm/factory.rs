use std::sync::Arc;

use arc_swap::ArcSwap;
use reqwest::Client;
use tracing::info;

use crate::config::{Config, LlmBackend};
use crate::error::AppError;
use crate::llm::EmbeddingProvider;
use crate::llm::LlmProvider;

use super::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitBreakerLlmWrapper};
use super::embedding::{LocalEmbeddingProvider, OpenAiEmbeddingProvider};
use super::providers::llamacpp::LlamaCppProvider;
use super::providers::ollama::OllamaProvider;
use super::providers::openai::OpenAiProvider;

/// Factory that creates LLM providers from configuration.
///
/// Holds a live reference to the config via `ArcSwap` so that every call to
/// `create()` / `create_vision()` reads the latest config snapshot (including
/// API keys, model names, endpoints changed at runtime).
pub struct LlmProviderFactory {
    client: Client,
    config: Arc<ArcSwap<Config>>,
}

impl LlmProviderFactory {
    pub fn new(client: Client, config: Arc<ArcSwap<Config>>) -> Self {
        Self { client, config }
    }

    /// Create an embedding provider based on the active LLM backend.
    pub fn create_embedding(&self) -> Result<Arc<dyn EmbeddingProvider>, AppError> {
        let config = self.config.load();
        match config.llm_backend {
            LlmBackend::Openai => {
                if config.openai_api_key.is_empty() {
                    return Err(AppError::Config(
                        "BOBE_OPENAI_API_KEY is required for OpenAI embedding backend".into(),
                    ));
                }
                let model = resolve_openai_embedding_model(&config.embedding_model);
                Ok(Arc::new(OpenAiEmbeddingProvider::openai(
                    self.client.clone(),
                    &config.openai_api_key,
                    &model,
                    config.embedding_dimension,
                )))
            }
            LlmBackend::AzureOpenai => {
                if config.azure_openai_endpoint.is_empty()
                    || config.azure_openai_api_key.is_empty()
                    || config.azure_openai_deployment.is_empty()
                {
                    return Err(AppError::Config(
                        "BOBE_AZURE_OPENAI_ENDPOINT, BOBE_AZURE_OPENAI_API_KEY, and BOBE_AZURE_OPENAI_DEPLOYMENT are required for Azure embedding backend".into(),
                    ));
                }
                Ok(Arc::new(OpenAiEmbeddingProvider::azure(
                    self.client.clone(),
                    &config.azure_openai_endpoint,
                    &config.azure_openai_api_key,
                    &config.azure_openai_deployment,
                    config.embedding_dimension,
                )))
            }
            _ => {
                let model = if config.embedding_model.trim().is_empty() {
                    "nomic-embed-text"
                } else {
                    &config.embedding_model
                };
                Ok(Arc::new(LocalEmbeddingProvider::new(
                    self.client.clone(),
                    &config.ollama_url,
                    model,
                    config.embedding_dimension,
                )))
            }
        }
    }

    /// Create a provider (wrapped with a circuit breaker) for the given backend string.
    ///
    /// Supported backends: `"ollama"`, `"openai"`, `"llamacpp"`, `"azure_openai"`.
    pub fn create(
        &self,
        backend: LlmBackend,
    ) -> Result<Arc<dyn LlmProvider>, crate::error::AppError> {
        let (provider, name) = self.create_raw(backend)?;

        let breaker = Arc::new(CircuitBreaker::new(
            format!("llm-{name}"),
            CircuitBreakerConfig::default(),
        ));

        info!(backend = name, "Created LLM provider with circuit breaker");

        Ok(Arc::new(CircuitBreakerLlmWrapper::new(provider, breaker)))
    }

    /// Create a vision-specific provider using vision model names from config.
    pub fn create_vision(
        &self,
        backend: LlmBackend,
    ) -> Result<Arc<dyn LlmProvider>, crate::error::AppError> {
        let config = self.config.load();

        let (provider, name): (Arc<dyn LlmProvider>, String) = match backend {
            LlmBackend::Ollama => {
                let p = OllamaProvider::new(
                    self.client.clone(),
                    &config.ollama_url,
                    &config.vision_ollama_model,
                );
                (Arc::new(p), "ollama-vision".into())
            }
            LlmBackend::Openai => {
                if config.openai_api_key.is_empty() {
                    return Err(crate::error::AppError::Config(
                        "BOBE_OPENAI_API_KEY is required for OpenAI vision backend".into(),
                    ));
                }
                let p = OpenAiProvider::new(
                    self.client.clone(),
                    &config.openai_api_key,
                    &config.vision_openai_model,
                );
                (Arc::new(p), "openai-vision".into())
            }
            LlmBackend::AzureOpenai => {
                if config.azure_openai_endpoint.is_empty() || config.azure_openai_api_key.is_empty()
                {
                    return Err(crate::error::AppError::Config(
                        "BOBE_AZURE_OPENAI_ENDPOINT and BOBE_AZURE_OPENAI_API_KEY required for Azure vision".into(),
                    ));
                }
                let deployment = if config.vision_azure_openai_deployment.is_empty() {
                    &config.azure_openai_deployment
                } else {
                    &config.vision_azure_openai_deployment
                };
                let p = OpenAiProvider::with_base_url(
                    self.client.clone(),
                    &config.azure_openai_endpoint,
                    &config.azure_openai_api_key,
                    deployment,
                );
                (Arc::new(p), "azure_openai-vision".into())
            }
            LlmBackend::LlamaCpp => {
                return Err(crate::error::AppError::Config(
                    "llama.cpp does not support vision models".into(),
                ));
            }
            LlmBackend::None => {
                return Err(crate::error::AppError::Config(
                    "Vision backend is disabled (set to 'none')".into(),
                ));
            }
        };

        let breaker = Arc::new(CircuitBreaker::new(
            format!("llm-{name}"),
            CircuitBreakerConfig::default(),
        ));

        info!(
            backend = name,
            "Created vision LLM provider with circuit breaker"
        );
        Ok(Arc::new(CircuitBreakerLlmWrapper::new(provider, breaker)))
    }

    /// Create a raw provider without circuit breaker wrapping.
    pub fn create_raw(
        &self,
        backend: LlmBackend,
    ) -> Result<(Arc<dyn LlmProvider>, String), crate::error::AppError> {
        let config = self.config.load();

        match backend {
            LlmBackend::Ollama => {
                let provider = OllamaProvider::new(
                    self.client.clone(),
                    &config.ollama_url,
                    &config.ollama_model,
                );
                Ok((Arc::new(provider), "ollama".into()))
            }
            LlmBackend::Openai => {
                if config.openai_api_key.is_empty() {
                    return Err(crate::error::AppError::Config(
                        "BOBE_OPENAI_API_KEY is required for OpenAI backend".into(),
                    ));
                }
                let provider = OpenAiProvider::new(
                    self.client.clone(),
                    &config.openai_api_key,
                    &config.openai_model,
                );
                Ok((Arc::new(provider), "openai".into()))
            }
            LlmBackend::LlamaCpp => {
                let provider =
                    LlamaCppProvider::new(self.client.clone(), &config.llama_url, "default");
                Ok((Arc::new(provider), "llamacpp".into()))
            }
            LlmBackend::AzureOpenai => {
                if config.azure_openai_endpoint.is_empty() || config.azure_openai_api_key.is_empty()
                {
                    return Err(crate::error::AppError::Config(
                        "BOBE_AZURE_OPENAI_ENDPOINT and BOBE_AZURE_OPENAI_API_KEY are required for Azure OpenAI backend".into(),
                    ));
                }
                if config.azure_openai_deployment.is_empty() {
                    return Err(crate::error::AppError::Config(
                        "BOBE_AZURE_OPENAI_DEPLOYMENT is required for Azure OpenAI backend".into(),
                    ));
                }
                let provider = OpenAiProvider::with_base_url(
                    self.client.clone(),
                    &config.azure_openai_endpoint,
                    &config.azure_openai_api_key,
                    &config.azure_openai_deployment,
                );
                Ok((Arc::new(provider), "azure_openai".into()))
            }
            LlmBackend::None => Err(crate::error::AppError::Config(
                "LLM backend is disabled (set to 'none')".into(),
            )),
        }
    }
}

fn resolve_openai_embedding_model(configured: &str) -> String {
    let model = configured.trim();
    if model.starts_with("text-embedding-") {
        model.to_string()
    } else {
        "text-embedding-3-small".to_string()
    }
}
