use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Canonical tool call — provider-agnostic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiToolCall {
    pub id: String,
    pub name: String,
    pub arguments: HashMap<String, serde_json::Value>,
}

/// Content can be text or multimodal parts (for vision).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Parts(Vec<serde_json::Value>),
}

impl MessageContent {
    pub fn as_text(&self) -> Option<&str> {
        match self {
            MessageContent::Text(s) => Some(s),
            MessageContent::Parts(_) => None,
        }
    }

    pub fn text_or_empty(&self) -> &str {
        self.as_text().unwrap_or("")
    }
}

impl From<String> for MessageContent {
    fn from(s: String) -> Self {
        MessageContent::Text(s)
    }
}

impl From<&str> for MessageContent {
    fn from(s: &str) -> Self {
        MessageContent::Text(s.to_owned())
    }
}

/// Canonical message format — all LLM providers map to/from this.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiMessage {
    pub role: String,
    pub content: MessageContent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<AiToolCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl AiMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: MessageContent::Text(content.into()),
            name: None,
            tool_calls: vec![],
            tool_call_id: None,
        }
    }

    pub fn user(content: impl Into<MessageContent>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
            name: None,
            tool_calls: vec![],
            tool_call_id: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: MessageContent::Text(content.into()),
            name: None,
            tool_calls: vec![],
            tool_call_id: None,
        }
    }

    pub fn assistant_with_tool_calls(tool_calls: Vec<AiToolCall>) -> Self {
        Self {
            role: "assistant".into(),
            content: MessageContent::Text(String::new()),
            name: None,
            tool_calls,
            tool_call_id: None,
        }
    }

    pub fn tool(tool_call_id: String, name: String, content: String) -> Self {
        Self {
            role: "tool".into(),
            content: MessageContent::Text(content),
            name: Some(name),
            tool_calls: vec![],
            tool_call_id: Some(tool_call_id),
        }
    }
}

/// Token usage statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Canonical LLM response.
#[derive(Debug, Clone)]
pub struct AiResponse {
    pub message: AiMessage,
    pub finish_reason: String,
    pub usage: Option<TokenUsage>,
}

/// Single chunk in a streaming response.
#[derive(Debug, Clone)]
pub struct StreamChunk {
    pub delta: String,
    pub tool_calls: Vec<AiToolCall>,
    pub finish_reason: Option<String>,
}

/// Tool definition for LLM function calling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// JSON Schema definition for structured output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonSchema {
    pub name: String,
    pub schema: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default = "default_strict")]
    pub strict: bool,
}

fn default_strict() -> bool {
    true
}

/// Response format specification for LLM output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseFormat {
    #[serde(rename = "type")]
    pub format_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_schema: Option<JsonSchema>,
}

impl ResponseFormat {
    pub fn text() -> Self {
        Self {
            format_type: "text".into(),
            json_schema: None,
        }
    }

    pub fn json() -> Self {
        Self {
            format_type: "json_object".into(),
            json_schema: None,
        }
    }

    pub fn structured(name: String, schema: serde_json::Value) -> Self {
        Self {
            format_type: "json_schema".into(),
            json_schema: Some(JsonSchema {
                name,
                schema,
                description: None,
                strict: true,
            }),
        }
    }
}
