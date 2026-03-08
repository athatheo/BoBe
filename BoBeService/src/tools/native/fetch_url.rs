use async_trait::async_trait;
use reqwest::redirect::Policy;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::net::IpAddr;

use super::base::NativeTool;
use crate::error::AppError;
use crate::tools::ToolExecutionContext;

/// SSRF blocklist: private, loopback, and link-local addresses.
fn is_private_host(host: &str) -> bool {
    #[allow(clippy::case_sensitive_file_extension_comparisons)]
    let is_local =
        host.eq_ignore_ascii_case("localhost") || host.to_ascii_lowercase().ends_with(".local");
    if is_local {
        return true;
    }

    if let Ok(ip) = host.parse::<IpAddr>() {
        return match ip {
            IpAddr::V4(v4) => {
                let octets = v4.octets();
                v4.is_loopback()                                     // 127.0.0.0/8
                    || octets[0] == 10                               // 10.0.0.0/8
                    || (octets[0] == 172 && (16..=31).contains(&octets[1])) // 172.16.0.0/12
                    || (octets[0] == 192 && octets[1] == 168)        // 192.168.0.0/16
                    || (octets[0] == 169 && octets[1] == 254)        // 169.254.0.0/16
                    || v4.is_unspecified() // 0.0.0.0
            }
            IpAddr::V6(v6) => {
                let segs = v6.segments();
                v6.is_loopback()                                     // ::1
                    || v6.is_unspecified()                            // ::
                    || (segs[0] & 0xffc0) == 0xfe80                  // fe80::/10 link-local
                    || (segs[0] & 0xfe00) == 0xfc00                  // fc00::/7  unique local
                    || v6.to_ipv4_mapped().is_some_and(|v4| {
                        let o = v4.octets();
                        v4.is_loopback()
                            || o[0] == 10
                            || (o[0] == 172 && (16..=31).contains(&o[1]))
                            || (o[0] == 192 && o[1] == 168)
                            || (o[0] == 169 && o[1] == 254)
                            || v4.is_unspecified()
                    })
            }
        };
    }

    false
}

pub(crate) struct FetchUrlTool {
    client: reqwest::Client,
}

impl Default for FetchUrlTool {
    fn default() -> Self {
        Self::new()
    }
}

impl FetchUrlTool {
    pub(crate) fn new() -> Self {
        // Redirect policy validates each hop against the SSRF blocklist
        let redirect_policy = Policy::custom(|attempt| {
            if attempt.previous().len() >= 10 {
                return attempt.error("too many redirects");
            }
            if let Some(host) = attempt.url().host_str()
                && is_private_host(host)
            {
                return attempt.error("redirect to private/internal address blocked");
            }
            attempt.follow()
        });

        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .redirect(redirect_policy)
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

    async fn execute(
        &self,
        arguments: HashMap<String, Value>,
        _context: Option<&ToolExecutionContext>,
    ) -> Result<String, AppError> {
        let url = arguments
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Validation("'url' is required".into()))?;

        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(AppError::Validation(
                "Only http:// and https:// URLs are allowed".into(),
            ));
        }

        let parsed: url::Url = url
            .parse()
            .map_err(|e| AppError::Validation(format!("Invalid URL: {e}")))?;

        if let Some(host) = parsed.host_str()
            && is_private_host(host)
        {
            return Err(AppError::Tool(
                "Access to private/internal addresses is blocked".into(),
            ));
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
        let is_html = content_type.contains("html");

        let mut content = if is_html {
            extract_visible_text(&body)
        } else {
            body.to_string()
        };

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

fn extract_visible_text(html: &str) -> String {
    use scraper::{Html, Selector};

    let document = Html::parse_document(html);
    let body_sel = Selector::parse("body").ok();

    let root = if let Some(ref sel) = body_sel {
        document.select(sel).next()
    } else {
        None
    };

    let mut out = String::new();
    let target = root.unwrap_or_else(|| document.root_element());
    collect_visible_text(&target, &mut out);
    out
}

const HIDDEN_TAGS: &[&str] = &["script", "style", "noscript", "svg", "template"];

fn collect_visible_text(node: &scraper::ElementRef<'_>, out: &mut String) {
    let tag = node.value().name();
    if HIDDEN_TAGS.contains(&tag) {
        return;
    }
    for child in node.children() {
        if let Some(text) = child.value().as_text() {
            let t = text.trim();
            if !t.is_empty() {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(t);
            }
        } else if let Some(el) = scraper::ElementRef::wrap(child) {
            collect_visible_text(&el, out);
        }
    }
}
