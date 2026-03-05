use serde_json::json;
use std::sync::LazyLock;

use crate::i18n::{FALLBACK_LOCALE, t, t_vars};
use crate::llm::types::{AiMessage, ResponseFormat};
use crate::runtime::prompts::base::PromptConfig;

pub static GOAL_EXTRACTION_SCHEMA: LazyLock<serde_json::Value> = LazyLock::new(|| {
    json!({
        "type": "object",
        "properties": {
            "goals": {
                "type": "array",
                "description": "List of inferred user goals",
                "items": {
                    "type": "object",
                    "properties": {
                        "content": {
                            "type": "string",
                            "description": "The goal statement - what the user wants to achieve"
                        },
                        "priority": {
                            "type": "string",
                            "enum": ["high", "medium", "low"],
                            "description": "'high' - urgent or frequently mentioned; 'medium' - important but not urgent; 'low' - nice to have or long-term"
                        },
                        "inference_reason": {
                            "type": "string",
                            "description": "Brief explanation of why this goal was inferred"
                        }
                    },
                    "required": ["content", "priority", "inference_reason"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["goals"],
        "additionalProperties": false
    })
});

pub struct GoalExtractionPrompt;

impl GoalExtractionPrompt {
    pub fn config() -> PromptConfig {
        PromptConfig {
            temperature: 0.2,
            max_tokens: 2048,
            response_format: Some(ResponseFormat::structured(
                "goal_extraction".into(),
                GOAL_EXTRACTION_SCHEMA.clone(),
            )),
            ..PromptConfig::default()
        }
    }

    pub fn messages(conversation_turns: &[String], existing_goals: &[String]) -> Vec<AiMessage> {
        let locale = FALLBACK_LOCALE;
        let conversation_text = conversation_turns.join("\n");

        let goals_text = if existing_goals.is_empty() {
            t(locale, "prompt-goal-extraction-no-existing-goals")
        } else {
            existing_goals
                .iter()
                .map(|g| format!("- {g}"))
                .collect::<Vec<_>>()
                .join("\n")
        };

        vec![
            AiMessage::system(t(locale, "prompt-goal-extraction-system")),
            AiMessage::user(t_vars(
                locale,
                "prompt-goal-extraction-user",
                &[
                    ("conversation_text", conversation_text),
                    ("goals_text", goals_text),
                ],
            )),
        ]
    }
}
