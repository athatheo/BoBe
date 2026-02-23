use std::pin::Pin;

use async_trait::async_trait;
use futures::{Stream, StreamExt};
use regex::Regex;
use reqwest::Client;
use tracing::{debug, error, warn};

use crate::adapters::llm::shared::{
    build_chat_request, parse_response, parse_stream_chunk,
};
use crate::error::AppError;
use crate::ports::llm::LlmProvider;
use crate::ports::llm_types::{
    AiMessage, AiResponse, ResponseFormat, StreamChunk, ToolDefinition,
};

/// Ollama LLM provider using the OpenAI-compatible /v1/chat/completions endpoint.
pub struct OllamaProvider {
    client: Client,
    base_url: String,
    model: String,
    think_tag_re: Regex,
}

impl OllamaProvider {
    pub fn new(client: Client, base_url: impl Into<String>, model: impl Into<String>) -> Self {
        let base_url = base_url.into().trim_end_matches('/').to_owned();
        Self {
            client,
            base_url,
            model: model.into(),
            think_tag_re: Regex::new(r"(?s)<think>.*?</think>").unwrap(),
        }
    }

    fn completions_url(&self) -> String {
        format!("{}/v1/chat/completions", self.base_url)
    }

    /// Strip `<think>...</think>` blocks from qwen3 model outputs.
    fn strip_think_tags(&self, text: &str) -> String {
        if self.model.contains("qwen3") {
            self.think_tag_re.replace_all(text, "").trim().to_owned()
        } else {
            text.to_owned()
        }
    }

    /// Check if currently inside a think block during streaming (for qwen3).
    fn is_qwen3(&self) -> bool {
        self.model.contains("qwen3")
    }
}

#[async_trait]
impl LlmProvider for OllamaProvider {
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

        debug!(model = %self.model, "Ollama complete request");

        let resp = self
            .client
            .post(self.completions_url())
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    AppError::LlmTimeout(format!("Ollama timeout: {e}"))
                } else if e.is_connect() {
                    AppError::LlmUnavailable(format!("Ollama unreachable: {e}"))
                } else {
                    AppError::Llm(format!("Ollama request failed: {e}"))
                }
            })?;

        let status = resp.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(AppError::LlmRateLimited("Ollama rate limited".into()));
        }
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            return Err(AppError::Llm(format!(
                "Ollama returned {status}: {body_text}"
            )));
        }

        let data: serde_json::Value = resp.json().await.map_err(|e| {
            AppError::Llm(format!("Failed to parse Ollama response: {e}"))
        })?;

        let mut response = parse_response(&data)?;
        // Strip think tags from qwen3 models
        if let crate::ports::llm_types::MessageContent::Text(ref mut text) =
            response.message.content
        {
            *text = self.strip_think_tags(text);
        }

        Ok(response)
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
        let is_qwen3 = self.is_qwen3();

        Box::pin(async_stream::stream! {
            let resp = match client.post(&url).json(&body).send().await {
                Ok(r) => r,
                Err(e) => {
                    let err = if e.is_timeout() {
                        AppError::LlmTimeout(format!("Ollama stream timeout: {e}"))
                    } else if e.is_connect() {
                        AppError::LlmUnavailable(format!("Ollama unreachable: {e}"))
                    } else {
                        AppError::Llm(format!("Ollama stream failed: {e}"))
                    };
                    yield Err(err);
                    return;
                }
            };

            if !resp.status().is_success() {
                let status = resp.status();
                let body_text = resp.text().await.unwrap_or_default();
                yield Err(AppError::Llm(format!(
                    "Ollama stream returned {status}: {body_text}"
                )));
                return;
            }

            let mut byte_stream = resp.bytes_stream();
            let mut buffer = String::new();
            let mut inside_think = false;

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

                while let Some(line_end) = buffer.find('\n') {
                    let line = buffer[..line_end].trim().to_owned();
                    buffer = buffer[line_end + 1..].to_owned();

                    if line.is_empty() || line == "data: [DONE]" {
                        continue;
                    }

                    let json_str = line.strip_prefix("data: ").unwrap_or(&line);

                    let data: serde_json::Value = match serde_json::from_str(json_str) {
                        Ok(d) => d,
                        Err(e) => {
                            warn!("Failed to parse SSE chunk: {e}");
                            continue;
                        }
                    };

                    if let Some(mut chunk) = parse_stream_chunk(&data) {
                        if is_qwen3 {
                            chunk.delta = filter_think_tags_streaming(
                                &chunk.delta,
                                &mut inside_think,
                            );
                        }
                        if !chunk.delta.is_empty()
                            || !chunk.tool_calls.is_empty()
                            || chunk.finish_reason.is_some()
                        {
                            yield Ok(chunk);
                        }
                    }
                }
            }
        })
    }

    async fn health_check(&self) -> bool {
        let url = format!("{}/api/tags", self.base_url);
        match self.client.get(&url).send().await {
            Ok(resp) => resp.status().is_success(),
            Err(e) => {
                error!("Ollama health check failed: {e}");
                false
            }
        }
    }

    fn supports_vision(&self) -> bool {
        let m = self.model.to_lowercase();
        m.contains("-vl") || m.contains("vision") || m.contains("llava")
    }

    fn supports_tools(&self) -> bool {
        true
    }
}

/// Filter `<think>...</think>` tags from streaming text, tracking state across chunks.
fn filter_think_tags_streaming(delta: &str, inside_think: &mut bool) -> String {
    let mut result = String::new();
    let mut chars = delta.chars().peekable();

    while let Some(&ch) = chars.peek() {
        if *inside_think {
            // Look for </think>
            let remaining: String = chars.clone().collect();
            if remaining.starts_with("</think>") {
                // Skip past the closing tag
                for _ in 0.."</think>".len() {
                    chars.next();
                }
                *inside_think = false;
            } else {
                chars.next();
            }
        } else {
            let remaining: String = chars.clone().collect();
            if remaining.starts_with("<think>") {
                for _ in 0.."<think>".len() {
                    chars.next();
                }
                *inside_think = true;
            } else {
                result.push(ch);
                chars.next();
            }
        }
    }

    result
}
