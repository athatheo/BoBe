use std::sync::Arc;

use reqwest::Client;
use tracing::info;

use crate::config::Config;
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
    pub fn create(&self, backend: &str) -> Result<Arc<dyn LlmProvider>, crate::error::AppError> {
        let (provider, name) = self.create_raw(backend)?;

        let breaker = Arc::new(CircuitBreaker::new(
            format!("llm-{name}"),
            CircuitBreakerConfig::default(),
        ));

        info!(backend = name, "Created LLM provider with circuit breaker");

        Ok(Arc::new(CircuitBreakerLlmWrapper::new(provider, breaker)))
    }

    /// Create a vision-specific provider using vision model names from config.
    pub fn create_vision(&self, backend: &str) -> Result<Arc<dyn LlmProvider>, crate::error::AppError> {
        let (provider, name): (Arc<dyn LlmProvider>, String) = match backend.to_lowercase().as_str() {
            "ollama" => {
                let p = OllamaProvider::new(
                    self.client.clone(),
                    &self.config.ollama_url,
                    &self.config.vision_ollama_model,
                );
                (Arc::new(p), "ollama-vision".into())
            }
            "openai" => {
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
            other => {
                return Err(crate::error::AppError::Config(format!(
                    "Unknown vision backend: '{other}'. Supported: ollama, openai"
                )));
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
        backend: &str,
    ) -> Result<(Arc<dyn LlmProvider>, String), crate::error::AppError> {
        match backend.to_lowercase().as_str() {
            "ollama" => {
                let provider = OllamaProvider::new(
                    self.client.clone(),
                    &self.config.ollama_url,
                    &self.config.ollama_model,
                );
                Ok((Arc::new(provider), "ollama".into()))
            }
            "openai" => {
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
            "llamacpp" | "llama_cpp" | "llama.cpp" => {
                let provider = LlamaCppProvider::new(
                    self.client.clone(),
                    &self.config.llama_url,
                    "default",
                );
                Ok((Arc::new(provider), "llamacpp".into()))
            }
            other => Err(crate::error::AppError::Config(format!(
                "Unknown LLM backend: '{other}'. Supported: ollama, openai, llamacpp"
            ))),
        }
    }
}
