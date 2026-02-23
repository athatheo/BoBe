use std::sync::Arc;

use async_trait::async_trait;
use futures::{Stream, StreamExt};
use std::pin::Pin;
use tokio::sync::Mutex;
use tokio::time::Instant;
use tracing::{info, warn};

use crate::error::AppError;
use crate::ports::llm::LlmProvider;
use crate::ports::llm_types::{
    AiMessage, AiResponse, ResponseFormat, StreamChunk, ToolDefinition,
};

/// Circuit breaker states following the standard pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

impl std::fmt::Display for CircuitState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitState::Closed => write!(f, "closed"),
            CircuitState::Open => write!(f, "open"),
            CircuitState::HalfOpen => write!(f, "half_open"),
        }
    }
}

/// Configuration for the circuit breaker.
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub recovery_timeout: std::time::Duration,
    pub half_open_max_calls: u32,
    pub backoff_multiplier: f64,
    pub max_recovery_timeout: std::time::Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 3,
            recovery_timeout: std::time::Duration::from_secs(30),
            half_open_max_calls: 1,
            backoff_multiplier: 2.0,
            max_recovery_timeout: std::time::Duration::from_secs(300),
        }
    }
}

struct CircuitBreakerInner {
    state: CircuitState,
    failure_count: u32,
    half_open_calls: u32,
    last_failure_time: Option<Instant>,
    current_recovery_timeout: std::time::Duration,
    config: CircuitBreakerConfig,
}

/// Async-safe circuit breaker using tokio::sync::Mutex.
pub struct CircuitBreaker {
    inner: Mutex<CircuitBreakerInner>,
    name: String,
}

impl CircuitBreaker {
    pub fn new(name: impl Into<String>, config: CircuitBreakerConfig) -> Self {
        let recovery_timeout = config.recovery_timeout;
        Self {
            name: name.into(),
            inner: Mutex::new(CircuitBreakerInner {
                state: CircuitState::Closed,
                failure_count: 0,
                half_open_calls: 0,
                last_failure_time: None,
                current_recovery_timeout: recovery_timeout,
                config,
            }),
        }
    }

    /// Check if a request is allowed through the circuit breaker.
    pub async fn allow_request(&self) -> Result<(), AppError> {
        let mut inner = self.inner.lock().await;
        match inner.state {
            CircuitState::Closed => Ok(()),
            CircuitState::Open => {
                if let Some(last_failure) = inner.last_failure_time {
                    if last_failure.elapsed() >= inner.current_recovery_timeout {
                        inner.state = CircuitState::HalfOpen;
                        inner.half_open_calls = 0;
                        info!(
                            circuit = %self.name,
                            "Circuit breaker transitioning to half-open"
                        );
                        Ok(())
                    } else {
                        let remaining =
                            inner.current_recovery_timeout - last_failure.elapsed();
                        Err(AppError::CircuitOpen(format!(
                            "Circuit '{}' is open, retry after {:.1}s",
                            self.name,
                            remaining.as_secs_f64()
                        )))
                    }
                } else {
                    Err(AppError::CircuitOpen(format!(
                        "Circuit '{}' is open",
                        self.name
                    )))
                }
            }
            CircuitState::HalfOpen => {
                if inner.half_open_calls < inner.config.half_open_max_calls {
                    inner.half_open_calls += 1;
                    Ok(())
                } else {
                    Err(AppError::CircuitOpen(format!(
                        "Circuit '{}' is half-open, max probe calls reached",
                        self.name
                    )))
                }
            }
        }
    }

    /// Record a successful call — resets circuit to closed.
    pub async fn record_success(&self) {
        let mut inner = self.inner.lock().await;
        if inner.state != CircuitState::Closed {
            info!(
                circuit = %self.name,
                prev_state = %inner.state,
                "Circuit breaker closing after success"
            );
        }
        inner.state = CircuitState::Closed;
        inner.failure_count = 0;
        inner.half_open_calls = 0;
        inner.current_recovery_timeout = inner.config.recovery_timeout;
    }

    /// Record a failed call — may trip the circuit open.
    pub async fn record_failure(&self) {
        let mut inner = self.inner.lock().await;
        inner.failure_count += 1;
        inner.last_failure_time = Some(Instant::now());

        match inner.state {
            CircuitState::HalfOpen => {
                inner.current_recovery_timeout = std::time::Duration::from_secs_f64(
                    (inner.current_recovery_timeout.as_secs_f64()
                        * inner.config.backoff_multiplier)
                        .min(inner.config.max_recovery_timeout.as_secs_f64()),
                );
                inner.state = CircuitState::Open;
                warn!(
                    circuit = %self.name,
                    recovery_secs = inner.current_recovery_timeout.as_secs(),
                    "Circuit breaker re-opening from half-open (backoff)"
                );
            }
            CircuitState::Closed => {
                if inner.failure_count >= inner.config.failure_threshold {
                    inner.state = CircuitState::Open;
                    warn!(
                        circuit = %self.name,
                        failures = inner.failure_count,
                        "Circuit breaker opening after threshold reached"
                    );
                }
            }
            CircuitState::Open => {}
        }
    }

    /// How long until the next retry is allowed (None if closed).
    pub async fn time_until_retry(&self) -> Option<std::time::Duration> {
        let inner = self.inner.lock().await;
        match inner.state {
            CircuitState::Open => inner.last_failure_time.map(|t| {
                let elapsed = t.elapsed();
                if elapsed < inner.current_recovery_timeout {
                    inner.current_recovery_timeout - elapsed
                } else {
                    std::time::Duration::ZERO
                }
            }),
            _ => None,
        }
    }

    /// Get the current circuit state.
    pub async fn get_status(&self) -> CircuitState {
        self.inner.lock().await.state.clone()
    }

    /// Manually reset the circuit breaker to closed state.
    pub async fn reset(&self) {
        let mut inner = self.inner.lock().await;
        inner.state = CircuitState::Closed;
        inner.failure_count = 0;
        inner.half_open_calls = 0;
        inner.last_failure_time = None;
        inner.current_recovery_timeout = inner.config.recovery_timeout;
        info!(circuit = %self.name, "Circuit breaker manually reset");
    }
}

/// Wrapper that adds circuit breaker protection to any `LlmProvider`.
pub struct CircuitBreakerLlmWrapper {
    provider: Arc<dyn LlmProvider>,
    breaker: Arc<CircuitBreaker>,
}

impl CircuitBreakerLlmWrapper {
    pub fn new(provider: Arc<dyn LlmProvider>, breaker: Arc<CircuitBreaker>) -> Self {
        Self { provider, breaker }
    }

    pub fn circuit_breaker(&self) -> &Arc<CircuitBreaker> {
        &self.breaker
    }
}

#[async_trait]
impl LlmProvider for CircuitBreakerLlmWrapper {
    async fn complete(
        &self,
        messages: &[AiMessage],
        tools: Option<&[ToolDefinition]>,
        response_format: Option<&ResponseFormat>,
        temperature: f32,
        max_tokens: u32,
    ) -> Result<AiResponse, AppError> {
        self.breaker.allow_request().await?;

        match self
            .provider
            .complete(messages, tools, response_format, temperature, max_tokens)
            .await
        {
            Ok(resp) => {
                self.breaker.record_success().await;
                Ok(resp)
            }
            Err(e) => {
                self.breaker.record_failure().await;
                Err(e)
            }
        }
    }

    fn stream(
        &self,
        messages: Vec<AiMessage>,
        tools: Option<Vec<ToolDefinition>>,
        response_format: Option<ResponseFormat>,
        temperature: f32,
        max_tokens: u32,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamChunk, AppError>> + Send + '_>> {
        let breaker = self.breaker.clone();
        let provider = self.provider.clone();

        Box::pin(async_stream::stream! {
            if let Err(e) = breaker.allow_request().await {
                yield Err(e);
                return;
            }

            let mut inner = provider.stream(
                messages, tools, response_format, temperature, max_tokens,
            );
            let mut had_error = false;

            while let Some(chunk) = inner.next().await {
                if chunk.is_err() {
                    had_error = true;
                }
                yield chunk;
            }

            if had_error {
                breaker.record_failure().await;
            } else {
                breaker.record_success().await;
            }
        })
    }

    async fn health_check(&self) -> bool {
        self.provider.health_check().await
    }

    fn supports_vision(&self) -> bool {
        self.provider.supports_vision()
    }

    fn supports_tools(&self) -> bool {
        self.provider.supports_tools()
    }
}
