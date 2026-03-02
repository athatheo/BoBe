//! Goal-triggered decision prompts.

use serde_json::json;
use std::sync::LazyLock;

use crate::llm::types::{AiMessage, ResponseFormat};
use crate::runtime::prompts::base::{DEFAULT_SOUL, PromptConfig};

/// JSON Schema for structured goal decision output.
pub static GOAL_DECISION_SCHEMA: LazyLock<serde_json::Value> = LazyLock::new(|| {
    json!({
        "type": "object",
        "properties": {
            "decision": {
                "type": "string",
                "enum": ["idle", "reach_out"],
                "description": "'idle' - goal not relevant to current context or now isn't a good time; 'reach_out' - goal is actionable and user could benefit from help"
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

/// Prompt for deciding whether to proactively help with a user's goal.
pub struct GoalDecisionPrompt;

impl GoalDecisionPrompt {
    const SYSTEM_TEMPLATE: &str = r"{soul}

You are deciding whether to proactively reach out to help the user with one of their goals.
Respond with a JSON object containing your decision and reasoning.

Decision guidelines:

REACH_OUT when:
- The user's current activity is relevant to this goal
- You can offer specific, actionable help right now
- The timing feels natural (user at a breakpoint or transition)
- Significant time has passed since last discussing this goal

IDLE when:
- The user is focused on something unrelated to this goal
- Interrupting would be disruptive to their current flow
- You've recently discussed this goal and haven't seen new context
- The goal seems paused or deprioritized based on user activity

Being helpful means knowing when NOT to interrupt. Default to IDLE when uncertain.";

    pub fn config() -> PromptConfig {
        PromptConfig {
            temperature: 0.3,
            max_tokens: 200,
            response_format: Some(ResponseFormat::structured(
                "goal_decision_response".into(),
                GOAL_DECISION_SCHEMA.clone(),
            )),
            ..PromptConfig::default()
        }
    }

    pub fn messages(
        goal_content: &str,
        context_summary: &str,
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
                "{time_line}User's goal:\n\
                 {goal_content}\n\n\
                 Current context (what the user is doing):\n\
                 {context_summary}\n\n\
                 Should I reach out to help with this goal right now? Consider:\n\
                 - Is the current context relevant to this goal?\n\
                 - Would reaching out be helpful or disruptive?\n\
                 - Is now a good time to offer assistance?"
            )),
        ]
    }
}
