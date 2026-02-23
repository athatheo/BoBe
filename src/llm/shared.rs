use std::collections::HashMap;

use serde_json::{Value, json};

use crate::error::AppError;
use crate::llm::types::{
    AiMessage, AiResponse, AiToolCall, MessageContent, ResponseFormat, StreamChunk, TokenUsage,
    ToolDefinition,
};

/// Convert an `AiMessage` to the OpenAI chat-completions JSON format.
pub fn message_to_json(msg: &AiMessage) -> Value {
    let mut obj = serde_json::Map::new();
    obj.insert("role".into(), json!(msg.role));

    match &msg.content {
        MessageContent::Text(s) => {
            obj.insert("content".into(), json!(s));
        }
        MessageContent::Parts(parts) => {
            obj.insert("content".into(), json!(parts));
        }
    }

    if let Some(name) = &msg.name {
        obj.insert("name".into(), json!(name));
    }

    if !msg.tool_calls.is_empty() {
        let tc: Vec<Value> = msg
            .tool_calls
            .iter()
            .map(|tc| {
                json!({
                    "id": tc.id,
                    "type": "function",
                    "function": {
                        "name": tc.name,
                        "arguments": serde_json::to_string(&tc.arguments).unwrap_or_default(),
                    }
                })
            })
            .collect();
        obj.insert("tool_calls".into(), json!(tc));
    }

    if let Some(tool_call_id) = &msg.tool_call_id {
        obj.insert("tool_call_id".into(), json!(tool_call_id));
    }

    Value::Object(obj)
}

/// Build a full chat-completions request body.
pub fn build_chat_request(
    model: &str,
    messages: &[AiMessage],
    tools: Option<&[ToolDefinition]>,
    response_format: Option<&ResponseFormat>,
    temperature: f32,
    max_tokens: u32,
    stream: bool,
) -> Value {
    let msgs: Vec<Value> = messages.iter().map(message_to_json).collect();

    let mut body = json!({
        "model": model,
        "messages": msgs,
        "temperature": temperature,
        "max_tokens": max_tokens,
        "stream": stream,
    });

    if let Some(tools) = tools
        && !tools.is_empty()
    {
        let tool_defs: Vec<Value> = tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters,
                    }
                })
            })
            .collect();
        body["tools"] = json!(tool_defs);
    }

    if let Some(rf) = response_format {
        let mut fmt = json!({"type": rf.format_type});
        if let Some(schema) = &rf.json_schema {
            fmt["json_schema"] = json!({
                "name": schema.name,
                "schema": schema.schema,
                "strict": schema.strict,
            });
            if let Some(desc) = &schema.description {
                fmt["json_schema"]["description"] = json!(desc);
            }
        }
        body["response_format"] = fmt;
    }

    body
}

/// Parse tool_calls from a response choice's message object.
pub fn parse_tool_calls(message: &Value) -> Vec<AiToolCall> {
    let Some(tool_calls) = message.get("tool_calls").and_then(|v| v.as_array()) else {
        return vec![];
    };

    tool_calls
        .iter()
        .filter_map(|tc| {
            let id = tc.get("id")?.as_str()?.to_owned();
            let func = tc.get("function")?;
            let name = func.get("name")?.as_str()?.to_owned();
            let args_str = func.get("arguments")?.as_str().unwrap_or("{}");
            let arguments: HashMap<String, Value> =
                serde_json::from_str(args_str).unwrap_or_default();
            Some(AiToolCall {
                id,
                name,
                arguments,
            })
        })
        .collect()
}

/// Parse a full (non-streaming) chat-completions response.
pub fn parse_response(data: &Value) -> Result<AiResponse, AppError> {
    let choice = data
        .get("choices")
        .and_then(|c| c.get(0))
        .ok_or_else(|| AppError::Llm("No choices in response".into()))?;

    let message = choice
        .get("message")
        .ok_or_else(|| AppError::Llm("No message in choice".into()))?;

    let role = message
        .get("role")
        .and_then(|r| r.as_str())
        .unwrap_or("assistant")
        .to_owned();

    let content = message
        .get("content")
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_owned();

    let tool_calls = parse_tool_calls(message);

    let finish_reason = choice
        .get("finish_reason")
        .and_then(|f| f.as_str())
        .unwrap_or("stop")
        .to_owned();

    let usage = data.get("usage").map(|u| TokenUsage {
        prompt_tokens: u.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        completion_tokens: u
            .get("completion_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        total_tokens: u.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
    });

    Ok(AiResponse {
        message: AiMessage {
            role,
            content: MessageContent::Text(content),
            name: None,
            tool_calls,
            tool_call_id: None,
        },
        finish_reason,
        usage,
    })
}

/// Parse a single SSE chunk from a streaming chat-completions response.
///
/// Tool call arguments in streaming deltas are fragments — they are NOT parsed
/// into `AiToolCall` here. Instead use `ToolCallAccumulator` to reconstruct
/// complete tool calls across chunks.
pub fn parse_stream_chunk(data: &Value) -> Option<StreamChunk> {
    let choice = data.get("choices")?.get(0)?;
    let delta = choice.get("delta")?;

    let text = delta
        .get("content")
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_owned();

    // Don't parse tool_calls from deltas — they're fragments.
    // Callers should use ToolCallAccumulator instead.
    let tool_calls = vec![];

    let finish_reason = choice
        .get("finish_reason")
        .and_then(|f| f.as_str())
        .map(|s| s.to_owned());

    Some(StreamChunk {
        delta: text,
        tool_calls,
        finish_reason,
    })
}

/// Extract raw tool call delta info from an SSE chunk for accumulation.
pub fn extract_tool_call_deltas(data: &Value) -> Vec<ToolCallDelta> {
    let Some(choice) = data.get("choices").and_then(|c| c.get(0)) else {
        return vec![];
    };
    let Some(delta) = choice.get("delta") else {
        return vec![];
    };
    let Some(tool_calls) = delta.get("tool_calls").and_then(|v| v.as_array()) else {
        return vec![];
    };

    tool_calls
        .iter()
        .filter_map(|tc| {
            let index = tc.get("index")?.as_u64()? as usize;
            let id = tc.get("id").and_then(|v| v.as_str()).map(|s| s.to_owned());
            let func = tc.get("function");
            let name = func
                .and_then(|f| f.get("name"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_owned());
            let arguments = func
                .and_then(|f| f.get("arguments"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            Some(ToolCallDelta {
                index,
                id,
                name,
                arguments,
            })
        })
        .collect()
}

/// Raw tool call delta from a single SSE chunk.
#[derive(Debug)]
pub struct ToolCallDelta {
    pub index: usize,
    pub id: Option<String>,
    pub name: Option<String>,
    pub arguments: String,
}

/// Accumulates streaming tool call deltas into complete `AiToolCall` objects.
///
/// OpenAI/Azure send tool calls incrementally: the first chunk contains
/// the `id` and `name`, subsequent chunks append to `arguments`.
#[derive(Default)]
pub struct ToolCallAccumulator {
    pending: Vec<PendingToolCall>,
}

#[derive(Default)]
struct PendingToolCall {
    id: String,
    name: String,
    arguments: String,
}

impl ToolCallAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed a raw SSE data value. Extracts and buffers any tool call deltas.
    pub fn feed(&mut self, data: &Value) {
        for delta in extract_tool_call_deltas(data) {
            // Grow the pending vec if needed
            while self.pending.len() <= delta.index {
                self.pending.push(PendingToolCall::default());
            }
            let entry = &mut self.pending[delta.index];
            if let Some(id) = delta.id {
                entry.id = id;
            }
            if let Some(name) = delta.name {
                entry.name = name;
            }
            entry.arguments.push_str(&delta.arguments);
        }
    }

    /// Returns true if any tool calls have been accumulated.
    pub fn has_tool_calls(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Finalize and return complete `AiToolCall` objects.
    pub fn finish(self) -> Vec<AiToolCall> {
        self.pending
            .into_iter()
            .filter(|tc| !tc.id.is_empty() && !tc.name.is_empty())
            .map(|tc| {
                let arguments: HashMap<String, Value> =
                    serde_json::from_str(&tc.arguments).unwrap_or_default();
                AiToolCall {
                    id: tc.id,
                    name: tc.name,
                    arguments,
                }
            })
            .collect()
    }
}
