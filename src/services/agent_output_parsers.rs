//! Parsers for coding agent output formats.
//!
//! Each parser extracts a common result structure from agent-specific output.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;
use tracing::warn;

/// Maximum lines to include in text-based summary.
const TEXT_SUMMARY_MAX_LINES: usize = 30;

/// Parsed result from a coding agent's output.
#[derive(Debug, Clone, Default)]
pub struct AgentJobResult {
    pub summary: Option<String>,
    pub cost_usd: Option<f64>,
    /// Agent session ID (for resume/continuation).
    pub session_id: Option<String>,
    pub files_changed: Vec<String>,
    pub tools_used: Vec<String>,
    pub is_error: bool,
    pub error_detail: Option<String>,
}

/// Parse Claude Code stream-json (NDJSON) output.
pub fn parse_claude_ndjson(output_path: &Path) -> AgentJobResult {
    let mut result = AgentJobResult::default();

    if !output_path.exists() {
        result.is_error = true;
        result.error_detail = Some(format!("Output file not found: {}", output_path.display()));
        return result;
    }

    let content = match std::fs::read_to_string(output_path) {
        Ok(c) => c,
        Err(e) => {
            warn!(error = %e, path = %output_path.display(), "agent_output_parser.read_error");
            result.is_error = true;
            result.error_detail = Some(format!("Failed to read output: {e}"));
            return result;
        }
    };

    let mut last_result_line: Option<Value> = None;
    let mut session_id: Option<String> = None;
    let mut files_seen = BTreeSet::new();
    let mut tools_seen = BTreeSet::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let msg: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let msg_type = msg.get("type").and_then(|v| v.as_str()).unwrap_or("");

        if msg_type == "system"
            && let Some(sid) = msg.get("session_id").and_then(|v| v.as_str())
            && !sid.is_empty()
        {
            session_id = Some(sid.to_owned());
        }

        if msg_type == "result" {
            last_result_line = Some(msg.clone());
        }

        // Track tool calls from assistant messages
        if msg_type == "assistant" {
            extract_tool_calls(&msg, &mut tools_seen, &mut files_seen);
        }

        // Track file changes from tool results
        if msg_type == "user" {
            extract_file_changes(&msg, &mut files_seen);
        }
    }

    if let Some(ref rl) = last_result_line {
        result.summary = rl.get("result").and_then(|v| v.as_str()).map(String::from);

        if let Some(cost) = rl.get("total_cost_usd").and_then(Value::as_f64) {
            result.cost_usd = Some(cost);
        }

        let subtype = rl.get("subtype").and_then(|v| v.as_str()).unwrap_or("");

        if subtype != "success" {
            result.is_error = true;
            if let Some(errors) = rl.get("errors").and_then(|v| v.as_array()) {
                if errors.is_empty() {
                    result.error_detail = Some(subtype.to_owned());
                } else {
                    let joined: Vec<String> = errors.iter().map(ToString::to_string).collect();
                    result.error_detail = Some(joined.join("; "));
                }
            } else {
                result.error_detail = Some(subtype.to_owned());
            }
        }
    } else {
        result.summary = Some("Agent completed but no result message found in output.".into());
    }

    result.session_id = session_id;
    result.files_changed = files_seen.into_iter().collect();
    result.tools_used = tools_seen.into_iter().collect();
    result
}

fn extract_tool_calls(
    msg: &Value,
    tools_seen: &mut BTreeSet<String>,
    files_seen: &mut BTreeSet<String>,
) {
    let Some(message) = msg.get("message") else {
        return;
    };
    let Some(content) = message.get("content").and_then(|v| v.as_array()) else {
        return;
    };
    for block in content {
        if block.get("type").and_then(|v| v.as_str()) != Some("tool_use") {
            continue;
        }
        if let Some(name) = block.get("name").and_then(|v| v.as_str()) {
            if !name.is_empty() {
                tools_seen.insert(name.to_owned());
            }
            if (name == "Write" || name == "Edit")
                && let Some(path) = block
                    .get("input")
                    .and_then(|v| v.get("file_path"))
                    .and_then(|v| v.as_str())
                && !path.is_empty()
            {
                files_seen.insert(path.to_owned());
            }
        }
    }
}

fn extract_file_changes(msg: &Value, files_seen: &mut BTreeSet<String>) {
    let Some(message) = msg.get("message") else {
        return;
    };
    let Some(content) = message.get("content").and_then(|v| v.as_array()) else {
        return;
    };
    for block in content {
        let name = block.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if (name == "Write" || name == "Edit")
            && let Some(path) = block
                .get("input")
                .and_then(|v| v.get("file_path"))
                .and_then(|v| v.as_str())
            && !path.is_empty()
        {
            files_seen.insert(path.to_owned());
        }
    }
}

/// Parse plain text output from agents like Aider or OpenCode.
pub fn parse_text_output(output_path: &Path) -> AgentJobResult {
    let mut result = AgentJobResult::default();

    if !output_path.exists() {
        result.is_error = true;
        result.error_detail = Some(format!("Output file not found: {}", output_path.display()));
        return result;
    }

    match std::fs::read_to_string(output_path) {
        Ok(text) => {
            let lines: Vec<&str> = text.trim().lines().collect();
            if lines.is_empty() {
                result.summary = Some("Agent produced no output.".into());
            } else {
                let start = lines.len().saturating_sub(TEXT_SUMMARY_MAX_LINES);
                let summary_lines = &lines[start..];
                result.summary = Some(summary_lines.join("\n"));
            }
        }
        Err(e) => {
            warn!(error = %e, path = %output_path.display(), "agent_output_parser.read_error");
            result.is_error = true;
            result.error_detail = Some(format!("Failed to read output: {e}"));
        }
    }

    result
}
