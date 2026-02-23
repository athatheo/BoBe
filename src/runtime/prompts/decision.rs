//! Decision engine prompts.

use serde_json::json;
use std::sync::LazyLock;

use crate::runtime::prompts::base::{DEFAULT_SOUL, PromptConfig};
use crate::llm::types::{AiMessage, ResponseFormat};

/// JSON Schema for structured decision output.
pub static DECISION_SCHEMA: LazyLock<serde_json::Value> = LazyLock::new(|| {
    json!({
        "type": "object",
        "properties": {
            "decision": {
                "type": "string",
                "enum": ["idle", "reach_out", "need_more_info"],
                "description": "'idle' - stay quiet, user is focused or no value to add; 'reach_out' - user appears stuck or could benefit from help; 'need_more_info' - not enough context to decide"
            },
            "reasoning": {
                "type": "string",
                "description": "Brief explanation for the decision (1-2 sentences)."
            }
        },
        "required": ["decision", "reasoning"],
        "additionalProperties": false
    })
});

/// Prompt for deciding whether to proactively engage with the user.
pub struct DecisionPrompt;

impl DecisionPrompt {
    const SYSTEM_TEMPLATE: &str = r#"{soul}

You are deciding whether to proactively reach out to the user.
Respond with a JSON object containing your decision and reasoning.

Available context you can consider:
- Recent observations of user activity (screenshots, active windows)
- Stored memories about user preferences and past interactions
- Active goals the user is working toward
- Recent conversation history

Available tools for deeper context (if needed):
- search_memories: Find relevant memories by semantic search
- get_goals: Retrieve user's active goals
- get_recent_context: Get recent observations and activity

Decision guidelines:

REACH_OUT when:
- The user appears stuck on a problem (repeated errors, same file for a long time)
- You notice a pattern that suggests they might need help
- There's a natural breakpoint where assistance would be welcome
- You have something genuinely useful and specific to offer
- A user goal is relevant to their current activity and you can help
- Your soul instructions specify a time-based action for the current time (e.g. daily review)

IDLE when:
- The user is in a flow state and interruption would be disruptive
- You've recently reached out and they didn't engage
- The context doesn't suggest any clear way you could help
- The user appears to be in focused, productive work

NEED_MORE_INFO when:
- The context is too limited to understand what the user is doing
- You need more observations before making a good decision
- The situation is ambiguous and more data would help

Being helpful means knowing when NOT to interrupt. Default to IDLE when uncertain."#;

    pub fn config() -> PromptConfig {
        PromptConfig {
            temperature: 0.3,
            max_tokens: 150,
            response_format: Some(ResponseFormat::structured(
                "decision_response".into(),
                DECISION_SCHEMA.clone(),
            )),
            ..PromptConfig::default()
        }
    }

    pub fn messages(
        current: &str,
        context: &str,
        recent_messages: &str,
        soul: Option<&str>,
        current_time: Option<&str>,
    ) -> Vec<AiMessage> {
        let system_content = Self::SYSTEM_TEMPLATE.replace("{soul}", soul.unwrap_or(DEFAULT_SOUL));

        let time_line = match current_time {
            Some(t) => format!("Current time: {t}\n\n"),
            None => String::new(),
        };

        vec![
            AiMessage::system(system_content),
            AiMessage::user(format!(
                "{time_line}Current observation:\n\
                 {current}\n\n\
                 Similar past context:\n\
                 {context}\n\n\
                 Recent Sent messages:\n\
                 {recent_messages}\n\n\
                 Analyze this information and decide whether I should reach out to the user."
            )),
        ]
    }
}
