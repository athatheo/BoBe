use async_trait::async_trait;
use serde_json::{Value, json};
use std::collections::HashMap;

use super::base::NativeTool;
use crate::error::AppError;
use crate::tools::{ToolCategory, ToolExecutionContext};

pub struct FetchUrlTool {
    client: reqwest::Client,
}

impl Default for FetchUrlTool {
    fn default() -> Self {
        Self::new()
    }
}

impl FetchUrlTool {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .user_agent("BoBe/1.0")
                .build()
                .unwrap_or_default(),
        }
    }
}

const MAX_CONTENT_SIZE: usize = 5_242_880; // 5 MB
const MAX_OUTPUT_SIZE: usize = 51_200; // 50 KB

#[async_trait]
impl NativeTool for FetchUrlTool {
    fn name(&self) -> &str {
        "fetch_url"
    }

    fn description(&self) -> &str {
        "Fetch web content from a URL. Only HTTP/HTTPS allowed. Blocks private/internal IPs."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch (http:// or https://)"
                }
            },
            "required": ["url"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Research
    }

    async fn execute(
        &self,
        arguments: HashMap<String, Value>,
        _context: Option<&ToolExecutionContext>,
    ) -> Result<String, AppError> {
        let url = arguments
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Validation("'url' is required".into()))?;

        // Validate URL scheme
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(AppError::Validation(
                "Only http:// and https:// URLs are allowed".into(),
            ));
        }

        // Parse and check for SSRF (private IP ranges)
        let parsed: url::Url = url
            .parse()
            .map_err(|e| AppError::Validation(format!("Invalid URL: {e}")))?;

        if let Some(host) = parsed.host_str() {
            let is_private = host == "localhost"
                || host == "127.0.0.1"
                || host == "::1"
                || host == "0.0.0.0"
                || host.starts_with("10.")
                || host.starts_with("192.168.")
                || host.starts_with("172.")
                || host.ends_with(".local");

            if is_private {
                return Err(AppError::Tool(
                    "Access to private/internal addresses is blocked".into(),
                ));
            }
        }

        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| AppError::Tool(format!("HTTP request failed: {e}")))?;

        let status = response.status();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unknown")
            .to_owned();

        let bytes = response
            .bytes()
            .await
            .map_err(|e| AppError::Tool(format!("Failed to read response body: {e}")))?;

        if bytes.len() > MAX_CONTENT_SIZE {
            return Err(AppError::Tool(format!(
                "Response too large ({} bytes). Maximum is {} bytes.",
                bytes.len(),
                MAX_CONTENT_SIZE
            )));
        }

        let body = String::from_utf8_lossy(&bytes);
        let mut content = body.to_string();

        if content.len() > MAX_OUTPUT_SIZE {
            content.truncate(MAX_OUTPUT_SIZE);
            content.push_str("\n\n--- Content truncated ---");
        }

        Ok(format!(
            "URL: {url}\nStatus: {status}\nContent-Type: {content_type}\nSize: {} bytes\n\n{content}",
            bytes.len()
        ))
    }
}
