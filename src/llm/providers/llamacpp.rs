use std::pin::Pin;

use async_trait::async_trait;
use futures::{Stream, StreamExt};
use reqwest::Client;
use tracing::{debug, error};

use crate::error::AppError;
use crate::llm::LlmProvider;
use crate::llm::shared::{
    ToolCallAccumulator, build_chat_request, drain_sse_lines, parse_response, parse_stream_chunk,
};
use crate::llm::types::{AiMessage, AiResponse, ResponseFormat, StreamChunk, ToolDefinition};

/// llama.cpp server provider using its OpenAI-compatible endpoint.
pub struct LlamaCppProvider {
    client: Client,
    base_url: String,
    model: String,
}

impl LlamaCppProvider {
    pub fn new(client: Client, base_url: impl Into<String>, model: impl Into<String>) -> Self {
        let base_url = base_url.into().trim_end_matches('/').to_owned();
        Self {
            client,
            base_url,
            model: model.into(),
        }
    }

    fn completions_url(&self) -> String {
        format!("{}/v1/chat/completions", self.base_url)
    }
}

#[async_trait]
impl LlmProvider for LlamaCppProvider {
    async fn complete(
        &self,
        messages: &[AiMessage],
        tools: Option<&[ToolDefinition]>,
        response_format: Option<&ResponseFormat>,
        temperature: f32,
        max_tokens: u32,
    ) -> Result<AiResponse, AppError> {
        let body = build_chat_request(
            &self.model,
            messages,
            tools,
            response_format,
            temperature,
            max_tokens,
            false,
        );

        debug!(model = %self.model, "llama.cpp complete request");

        let resp = self
            .client
            .post(self.completions_url())
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    AppError::LlmTimeout(format!("llama.cpp timeout: {e}"))
                } else if e.is_connect() {
                    AppError::LlmUnavailable(format!("llama.cpp unreachable: {e}"))
                } else {
                    AppError::Llm(format!("llama.cpp request failed: {e}"))
                }
            })?;

        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            return Err(AppError::Llm(format!(
                "llama.cpp returned {status}: {body_text}"
            )));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::Llm(format!("Failed to parse llama.cpp response: {e}")))?;

        parse_response(&data)
    }

    fn stream(
        &self,
        messages: Vec<AiMessage>,
        tools: Option<Vec<ToolDefinition>>,
        response_format: Option<ResponseFormat>,
        temperature: f32,
        max_tokens: u32,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamChunk, AppError>> + Send + '_>> {
        let body = build_chat_request(
            &self.model,
            &messages,
            tools.as_deref(),
            response_format.as_ref(),
            temperature,
            max_tokens,
            true,
        );
        let url = self.completions_url();
        let client = self.client.clone();

        Box::pin(async_stream::stream! {
            let resp = match client.post(&url).json(&body).send().await {
                Ok(r) => r,
                Err(e) => {
                    let err = if e.is_timeout() {
                        AppError::LlmTimeout(format!("llama.cpp stream timeout: {e}"))
                    } else if e.is_connect() {
                        AppError::LlmUnavailable(format!("llama.cpp unreachable: {e}"))
                    } else {
                        AppError::Llm(format!("llama.cpp stream failed: {e}"))
                    };
                    yield Err(err);
                    return;
                }
            };

            if !resp.status().is_success() {
                let status = resp.status();
                let body_text = resp.text().await.unwrap_or_default();
                yield Err(AppError::Llm(format!(
                    "llama.cpp stream returned {status}: {body_text}"
                )));
                return;
            }

            let mut byte_stream = resp.bytes_stream();
            let mut buffer = String::new();
            let mut tool_accumulator = ToolCallAccumulator::new();

            while let Some(bytes_result) = byte_stream.next().await {
                let bytes = match bytes_result {
                    Ok(b) => b,
                    Err(e) => {
                        yield Err(AppError::Llm(format!("Stream read error: {e}")));
                        return;
                    }
                };

                buffer.push_str(&String::from_utf8_lossy(&bytes));

                if buffer.len() > 1_048_576 {
                    yield Err(AppError::Llm("SSE buffer exceeded 1MB limit".into()));
                    return;
                }

                for data in drain_sse_lines(&mut buffer, "llama.cpp") {
                    tool_accumulator.feed(&data);

                    if let Some(mut chunk) = parse_stream_chunk(&data) {
                        if chunk.finish_reason.is_some() && tool_accumulator.has_tool_calls() {
                            let acc = std::mem::take(&mut tool_accumulator);
                            chunk.tool_calls = acc.finish();
                        }
                        yield Ok(chunk);
                    }
                }
            }
        })
    }

    async fn health_check(&self) -> bool {
        let url = format!("{}/health", self.base_url);
        match self.client.get(&url).send().await {
            Ok(resp) => resp.status().is_success(),
            Err(e) => {
                error!("llama.cpp health check failed: {e}");
                false
            }
        }
    }

    fn supports_vision(&self) -> bool {
        false
    }

    fn supports_tools(&self) -> bool {
        true
    }
}
