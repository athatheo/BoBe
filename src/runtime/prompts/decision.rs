use serde_json::json;
use std::sync::LazyLock;

use crate::i18n::{FALLBACK_LOCALE, t_vars};
use crate::llm::types::{AiMessage, ResponseFormat};
use crate::runtime::prompts::base::{DEFAULT_SOUL, PromptConfig};

pub(crate) static DECISION_SCHEMA: LazyLock<serde_json::Value> = LazyLock::new(|| {
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

pub(crate) struct DecisionPrompt;

impl DecisionPrompt {
    pub(crate) fn config() -> PromptConfig {
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

    pub(crate) fn messages(
        current: &str,
        context: &str,
        recent_messages: &str,
        soul: Option<&str>,
        current_time: Option<&str>,
        locale: Option<&str>,
    ) -> Vec<AiMessage> {
        let locale = locale.unwrap_or(FALLBACK_LOCALE);
        let system_content = t_vars(
            locale,
            "prompt-decision-system",
            &[("soul", soul.unwrap_or(DEFAULT_SOUL).to_owned())],
        );

        let time_line = match current_time {
            Some(t) => format!(
                "{}\n\n",
                t_vars(
                    locale,
                    "prompt-decision-current-time",
                    &[("time", t.to_owned())],
                )
            ),
            None => String::new(),
        };

        vec![
            AiMessage::system(system_content),
            AiMessage::user(t_vars(
                locale,
                "prompt-decision-user",
                &[
                    ("time_line", time_line),
                    ("current", current.to_owned()),
                    ("context", context.to_owned()),
                    ("recent_messages", recent_messages.to_owned()),
                ],
            )),
        ]
    }
}
