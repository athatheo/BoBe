use std::collections::HashMap;

use serde_json::{json, Value};

use crate::error::AppError;
use crate::ports::llm_types::{
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
        && !tools.is_empty() {
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
        prompt_tokens: u
            .get("prompt_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        completion_tokens: u
            .get("completion_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        total_tokens: u
            .get("total_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
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
pub fn parse_stream_chunk(data: &Value) -> Option<StreamChunk> {
    let choice = data.get("choices")?.get(0)?;
    let delta = choice.get("delta")?;

    let text = delta
        .get("content")
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_owned();

    let tool_calls = parse_tool_calls(delta);

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
