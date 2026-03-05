use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AiToolCall {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) arguments: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum MessageContent {
    Text(String),
    Parts(Vec<serde_json::Value>),
}

impl MessageContent {
    pub(crate) fn as_text(&self) -> Option<&str> {
        match self {
            MessageContent::Text(s) => Some(s),
            MessageContent::Parts(_) => None,
        }
    }

    pub(crate) fn text_or_empty(&self) -> &str {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AiMessage {
    pub(crate) role: String,
    pub(crate) content: MessageContent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) tool_calls: Vec<AiToolCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tool_call_id: Option<String>,
}

impl AiMessage {
    pub(crate) fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: MessageContent::Text(content.into()),
            name: None,
            tool_calls: vec![],
            tool_call_id: None,
        }
    }

    pub(crate) fn user(content: impl Into<MessageContent>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
            name: None,
            tool_calls: vec![],
            tool_call_id: None,
        }
    }

    pub(crate) fn assistant_with_tool_calls(tool_calls: Vec<AiToolCall>) -> Self {
        Self {
            role: "assistant".into(),
            content: MessageContent::Text(String::new()),
            name: None,
            tool_calls,
            tool_call_id: None,
        }
    }

    pub(crate) fn tool(tool_call_id: String, name: String, content: String) -> Self {
        Self {
            role: "tool".into(),
            content: MessageContent::Text(content),
            name: Some(name),
            tool_calls: vec![],
            tool_call_id: Some(tool_call_id),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct TokenUsage {
    pub(crate) prompt_tokens: u32,
    pub(crate) completion_tokens: u32,
    pub(crate) total_tokens: u32,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct AiResponse {
    pub(crate) message: AiMessage,
    pub(crate) finish_reason: String,
    pub(crate) usage: Option<TokenUsage>,
}

#[derive(Debug, Clone)]
pub(crate) struct StreamChunk {
    pub(crate) delta: String,
    pub(crate) tool_calls: Vec<AiToolCall>,
    pub(crate) finish_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) enum StreamItem {
    Chunk(StreamChunk),
    TypedToolNotification(crate::tools::ToolNotification),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ToolDefinition {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct JsonSchema {
    pub(crate) name: String,
    pub(crate) schema: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,
    #[serde(default = "default_strict")]
    pub(crate) strict: bool,
}

fn default_strict() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ResponseFormat {
    #[serde(rename = "type")]
    pub(crate) format_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) json_schema: Option<JsonSchema>,
}

impl ResponseFormat {
    pub(crate) fn json() -> Self {
        Self {
            format_type: "json_object".into(),
            json_schema: None,
        }
    }

    pub(crate) fn structured(name: String, schema: serde_json::Value) -> Self {
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
