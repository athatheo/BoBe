use std::sync::Arc;

use arc_swap::ArcSwap;
use tracing::{debug, info, warn};

use crate::config::Config;
use crate::llm::LlmProvider;
use crate::llm::types::{AiMessage, ToolDefinition};

const PRESELECTOR_SYSTEM_PROMPT: &str = r#"You are a tool selection assistant. Given a conversation and a list of available tools, select which tools are most likely to be needed.

Return a JSON object with a single key "tools" containing an array of tool names.
Example: {"tools": ["search_memories", "file_reader"]}

Rules:
- Err on the side of inclusion — it's better to include an extra tool than miss one
- Consider the full conversation context, not just the last message
- If unsure, include the tool
- Return at least 1 tool if there's any conversation content"#;

const MAX_CONVERSATION_MESSAGES: usize = 10;
const MAX_TOKENS: u32 = 500;

/// Narrows the tool list based on conversation context using an LLM call.
pub struct ToolPreselector {
    llm: Arc<dyn LlmProvider>,
    config: Arc<ArcSwap<Config>>,
    max_tools_for_bypass: usize,
}

impl ToolPreselector {
    pub fn new(llm: Arc<dyn LlmProvider>, config: Arc<ArcSwap<Config>>) -> Self {
        Self {
            llm,
            config,
            max_tools_for_bypass: 5,
        }
    }

    /// Select relevant tools based on conversation context.
    ///
    /// Returns all tools if preselection is disabled, there are few tools, or on error.
    pub async fn preselect(
        &self,
        messages: &[AiMessage],
        all_tools: &[ToolDefinition],
    ) -> Vec<ToolDefinition> {
        let cfg = self.config.load();

        // Bypass conditions
        if !cfg.tools.preselector_enabled
            || all_tools.len() <= self.max_tools_for_bypass
            || messages.is_empty()
        {
            return all_tools.to_vec();
        }

        debug!(
            tool_count = all_tools.len(),
            message_count = messages.len(),
            "Running tool preselection"
        );

        let tool_list = format_tool_list(all_tools);
        let conversation = format_conversation(messages);

        let user_prompt = format!(
            "## Available Tools\n\n{tool_list}\n\n## Recent Conversation\n\n{conversation}\n\n\
             Select the tools most likely needed for this conversation. Return JSON only."
        );

        let preselector_messages = vec![
            AiMessage::system(PRESELECTOR_SYSTEM_PROMPT),
            AiMessage::user(user_prompt),
        ];

        let response = tokio::time::timeout(
            std::time::Duration::from_secs(240),
            self.llm
                .complete(&preselector_messages, None, None, 0.0, MAX_TOKENS),
        )
        .await;

        match response {
            Ok(Ok(ai_response)) => {
                let content = ai_response.message.content.text_or_empty().to_owned();
                match parse_tool_names(&content) {
                    Some(names) => {
                        let selected: Vec<ToolDefinition> = all_tools
                            .iter()
                            .filter(|t| names.contains(&t.name))
                            .cloned()
                            .collect();

                        if selected.is_empty() {
                            warn!("Preselector returned no valid tools, using all");
                            all_tools.to_vec()
                        } else {
                            info!(
                                selected = selected.len(),
                                total = all_tools.len(),
                                "Tool preselection completed"
                            );
                            selected
                        }
                    }
                    None => {
                        warn!("Failed to parse preselector response, using all tools");
                        all_tools.to_vec()
                    }
                }
            }
            Ok(Err(e)) => {
                warn!(error = %e, "Preselector LLM call failed, using all tools");
                all_tools.to_vec()
            }
            Err(_) => {
                warn!("Preselector timed out, using all tools");
                all_tools.to_vec()
            }
        }
    }
}

fn format_tool_list(tools: &[ToolDefinition]) -> String {
    tools
        .iter()
        .map(|t| format!("- **{}**: {}", t.name, t.description))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_conversation(messages: &[AiMessage]) -> String {
    let recent = if messages.len() > MAX_CONVERSATION_MESSAGES {
        &messages[messages.len() - MAX_CONVERSATION_MESSAGES..]
    } else {
        messages
    };

    recent
        .iter()
        .map(|m| {
            let content = m.content.text_or_empty();
            let truncated = if content.len() > 500 {
                format!("{}...", crate::util::text::truncate_str(content, 500))
            } else {
                content.to_owned()
            };
            format!("**{}**: {}", m.role, truncated)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn parse_tool_names(content: &str) -> Option<Vec<String>> {
    // Try parsing as JSON
    let trimmed = content.trim();

    // Handle markdown code blocks
    let json_str = if trimmed.starts_with("```") {
        trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
    } else {
        trimmed
    };

    let parsed: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let tools = parsed.get("tools")?.as_array()?;
    let names: Vec<String> = tools
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_owned()))
        .collect();

    if names.is_empty() { None } else { Some(names) }
}
