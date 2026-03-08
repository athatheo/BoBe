use serde_json::json;
use std::sync::LazyLock;

use crate::i18n::{FALLBACK_LOCALE, t_vars};
use crate::llm::types::{AiMessage, ResponseFormat};
use crate::runtime::prompts::base::{DEFAULT_SOUL, PromptConfig};

pub(crate) static GOAL_DECISION_SCHEMA: LazyLock<serde_json::Value> = LazyLock::new(|| {
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

pub(crate) struct GoalDecisionPrompt;

impl GoalDecisionPrompt {
    pub(crate) fn config() -> PromptConfig {
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

    pub(crate) fn messages(
        goal_content: &str,
        context_summary: &str,
        soul: Option<&str>,
        current_time: Option<&str>,
        locale: Option<&str>,
    ) -> Vec<AiMessage> {
        let locale = locale.unwrap_or(FALLBACK_LOCALE);
        let system_content = t_vars(
            locale,
            "prompt-goal-decision-system",
            &[("soul", soul.unwrap_or(DEFAULT_SOUL).to_owned())],
        );

        let time_line = match current_time {
            Some(t) => format!(
                "{}\n\n",
                t_vars(
                    locale,
                    "prompt-goal-decision-current-time",
                    &[("time", t.to_owned())],
                )
            ),
            None => String::new(),
        };

        vec![
            AiMessage::system(system_content),
            AiMessage::user(t_vars(
                locale,
                "prompt-goal-decision-user",
                &[
                    ("time_line", time_line),
                    ("goal_content", goal_content.to_owned()),
                    ("context_summary", context_summary.to_owned()),
                ],
            )),
        ]
    }
}
