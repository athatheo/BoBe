use std::pin::Pin;

use async_trait::async_trait;
use futures::{Stream, StreamExt};
use reqwest::Client;
use tracing::{debug, error, warn};

use crate::llm::shared::{
    ToolCallAccumulator, build_chat_request, parse_response, parse_stream_chunk,
};
use crate::error::AppError;
use crate::llm::LlmProvider;
use crate::llm::types::{AiMessage, AiResponse, ResponseFormat, StreamChunk, ToolDefinition};

/// OpenAI API provider.
pub struct OpenAiProvider {
    client: Client,
    base_url: String,
    api_key: String,
    model: String,
}

impl OpenAiProvider {
    pub fn new(client: Client, api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            client,
            base_url: "https://api.openai.com".to_owned(),
            api_key: api_key.into(),
            model: model.into(),
        }
    }

    pub fn with_base_url(
        client: Client,
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        let base_url = base_url.into().trim_end_matches('/').to_owned();
        Self {
            client,
            base_url,
            api_key: api_key.into(),
            model: model.into(),
        }
    }

    fn completions_url(&self) -> String {
        format!("{}/v1/chat/completions", self.base_url)
    }
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
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

        debug!(model = %self.model, "OpenAI complete request");

        let resp = self
            .client
            .post(self.completions_url())
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    AppError::LlmTimeout(format!("OpenAI timeout: {e}"))
                } else if e.is_connect() {
                    AppError::LlmUnavailable(format!("OpenAI unreachable: {e}"))
                } else {
                    AppError::Llm(format!("OpenAI request failed: {e}"))
                }
            })?;

        let status = resp.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(AppError::LlmRateLimited("OpenAI rate limited".into()));
        }
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(AppError::Llm("OpenAI: invalid API key".into()));
        }
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            return Err(AppError::Llm(format!(
                "OpenAI returned {status}: {body_text}"
            )));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::Llm(format!("Failed to parse OpenAI response: {e}")))?;

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
        let api_key = self.api_key.clone();

        Box::pin(async_stream::stream! {
            let resp = match client
                .post(&url)
                .header("Authorization", format!("Bearer {}", api_key))
                .json(&body)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    let err = if e.is_timeout() {
                        AppError::LlmTimeout(format!("OpenAI stream timeout: {e}"))
                    } else if e.is_connect() {
                        AppError::LlmUnavailable(format!("OpenAI unreachable: {e}"))
                    } else {
                        AppError::Llm(format!("OpenAI stream failed: {e}"))
                    };
                    yield Err(err);
                    return;
                }
            };

            let status = resp.status();
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                yield Err(AppError::LlmRateLimited("OpenAI rate limited".into()));
                return;
            }
            if !status.is_success() {
                let body_text = resp.text().await.unwrap_or_default();
                yield Err(AppError::Llm(format!(
                    "OpenAI stream returned {status}: {body_text}"
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

                // Guard against unbounded buffer growth from malformed streams
                if buffer.len() > 1_048_576 {
                    yield Err(AppError::Llm("SSE buffer exceeded 1MB limit".into()));
                    return;
                }

                while let Some(line_end) = buffer.find('\n') {
                    // Parse inside a block so borrows are dropped before drain
                    let data: Option<serde_json::Value> = {
                        let line = buffer[..line_end].trim();
                        if line.is_empty() || line == "data: [DONE]" {
                            None
                        } else {
                            let json_str = line.strip_prefix("data: ").unwrap_or(line);
                            match serde_json::from_str(json_str) {
                                Ok(d) => Some(d),
                                Err(e) => {
                                    warn!("Failed to parse OpenAI SSE chunk: {e}");
                                    None
                                }
                            }
                        }
                    };
                    buffer.drain(..line_end + 1);

                    let Some(data) = data else { continue };

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
        match self
            .client
            .get(format!("{}/v1/models", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
        {
            Ok(resp) => resp.status().is_success(),
            Err(e) => {
                error!("OpenAI health check failed: {e}");
                false
            }
        }
    }

    fn supports_vision(&self) -> bool {
        let m = self.model.to_lowercase();
        m.contains("gpt-4o") || m.contains("gpt-4-vision") || m.contains("gpt-4-turbo")
    }

    fn supports_tools(&self) -> bool {
        true
    }
}
