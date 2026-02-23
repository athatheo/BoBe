use std::sync::Arc;

use reqwest::Client;
use tracing::info;

use crate::config::{Config, LlmBackend};
use crate::ports::llm::LlmProvider;

use super::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitBreakerLlmWrapper};
use super::providers::llamacpp::LlamaCppProvider;
use super::providers::ollama::OllamaProvider;
use super::providers::openai::OpenAiProvider;

/// Factory that creates LLM providers from configuration.
pub struct LlmProviderFactory {
    client: Client,
    config: Config,
}

impl LlmProviderFactory {
    pub fn new(client: Client, config: Config) -> Self {
        Self { client, config }
    }

    /// Create a provider (wrapped with a circuit breaker) for the given backend string.
    ///
    /// Supported backends: `"ollama"`, `"openai"`, `"llamacpp"`.
    pub fn create(&self, backend: LlmBackend) -> Result<Arc<dyn LlmProvider>, crate::error::AppError> {
        let (provider, name) = self.create_raw(backend)?;

        let breaker = Arc::new(CircuitBreaker::new(
            format!("llm-{name}"),
            CircuitBreakerConfig::default(),
        ));

        info!(backend = name, "Created LLM provider with circuit breaker");

        Ok(Arc::new(CircuitBreakerLlmWrapper::new(provider, breaker)))
    }

    /// Create a vision-specific provider using vision model names from config.
    pub fn create_vision(&self, backend: LlmBackend) -> Result<Arc<dyn LlmProvider>, crate::error::AppError> {
        let (provider, name): (Arc<dyn LlmProvider>, String) = match backend {
            LlmBackend::Ollama => {
                let p = OllamaProvider::new(
                    self.client.clone(),
                    &self.config.ollama_url,
                    &self.config.vision_ollama_model,
                );
                (Arc::new(p), "ollama-vision".into())
            }
            LlmBackend::Openai => {
                if self.config.openai_api_key.is_empty() {
                    return Err(crate::error::AppError::Config(
                        "BOBE_OPENAI_API_KEY is required for OpenAI vision backend".into(),
                    ));
                }
                let p = OpenAiProvider::new(
                    self.client.clone(),
                    &self.config.openai_api_key,
                    &self.config.vision_openai_model,
                );
                (Arc::new(p), "openai-vision".into())
            }
            LlmBackend::AzureOpenai => {
                if self.config.azure_openai_endpoint.is_empty() || self.config.azure_openai_api_key.is_empty() {
                    return Err(crate::error::AppError::Config(
                        "BOBE_AZURE_OPENAI_ENDPOINT and BOBE_AZURE_OPENAI_API_KEY required for Azure vision".into(),
                    ));
                }
                let deployment = if self.config.vision_azure_openai_deployment.is_empty() {
                    &self.config.azure_openai_deployment
                } else {
                    &self.config.vision_azure_openai_deployment
                };
                let p = OpenAiProvider::with_base_url(
                    self.client.clone(),
                    &self.config.azure_openai_endpoint,
                    &self.config.azure_openai_api_key,
                    deployment,
                );
                (Arc::new(p), "azure_openai-vision".into())
            }
            LlmBackend::LlamaCpp => {
                return Err(crate::error::AppError::Config(
                    "llama.cpp does not support vision models".into(),
                ));
            }
        };

        let breaker = Arc::new(CircuitBreaker::new(
            format!("llm-{name}"),
            CircuitBreakerConfig::default(),
        ));

        info!(backend = name, "Created vision LLM provider with circuit breaker");
        Ok(Arc::new(CircuitBreakerLlmWrapper::new(provider, breaker)))
    }

    /// Create a raw provider without circuit breaker wrapping.
    pub fn create_raw(
        &self,
        backend: LlmBackend,
    ) -> Result<(Arc<dyn LlmProvider>, String), crate::error::AppError> {
        match backend {
            LlmBackend::Ollama => {
                let provider = OllamaProvider::new(
                    self.client.clone(),
                    &self.config.ollama_url,
                    &self.config.ollama_model,
                );
                Ok((Arc::new(provider), "ollama".into()))
            }
            LlmBackend::Openai => {
                if self.config.openai_api_key.is_empty() {
                    return Err(crate::error::AppError::Config(
                        "BOBE_OPENAI_API_KEY is required for OpenAI backend".into(),
                    ));
                }
                let provider = OpenAiProvider::new(
                    self.client.clone(),
                    &self.config.openai_api_key,
                    &self.config.openai_model,
                );
                Ok((Arc::new(provider), "openai".into()))
            }
            LlmBackend::LlamaCpp => {
                let provider = LlamaCppProvider::new(
                    self.client.clone(),
                    &self.config.llama_url,
                    "default",
                );
                Ok((Arc::new(provider), "llamacpp".into()))
            }
            LlmBackend::AzureOpenai => {
                if self.config.azure_openai_endpoint.is_empty() || self.config.azure_openai_api_key.is_empty() {
                    return Err(crate::error::AppError::Config(
                        "BOBE_AZURE_OPENAI_ENDPOINT and BOBE_AZURE_OPENAI_API_KEY are required for Azure OpenAI backend".into(),
                    ));
                }
                if self.config.azure_openai_deployment.is_empty() {
                    return Err(crate::error::AppError::Config(
                        "BOBE_AZURE_OPENAI_DEPLOYMENT is required for Azure OpenAI backend".into(),
                    ));
                }
                let provider = OpenAiProvider::with_base_url(
                    self.client.clone(),
                    &self.config.azure_openai_endpoint,
                    &self.config.azure_openai_api_key,
                    &self.config.azure_openai_deployment,
                );
                Ok((Arc::new(provider), "azure_openai".into()))
            }
        }
    }
}
