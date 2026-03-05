use serde_json::json;
use std::sync::LazyLock;

use crate::constants::VALID_MEMORY_CATEGORIES;
use crate::i18n::{FALLBACK_LOCALE, t, t_vars};
use crate::llm::types::{AiMessage, ResponseFormat};
use crate::runtime::prompts::base::PromptConfig;

pub static MEMORY_EXTRACTION_SCHEMA: LazyLock<serde_json::Value> = LazyLock::new(|| {
    json!({
        "type": "object",
        "properties": {
            "memories": {
                "type": "array",
                "description": "List of extracted memories from the context",
                "items": {
                    "type": "object",
                    "properties": {
                        "content": {
                            "type": "string",
                            "description": "The memory content - a factual statement about the user"
                        },
                        "category": {
                            "type": "string",
                            "enum": VALID_MEMORY_CATEGORIES,
                            "description": "'preference' - user likes/dislikes something; 'pattern' - recurring behavior or habit; 'fact' - biographical or contextual info; 'interest' - topic the user is interested in"
                        },
                        "importance": {
                            "type": "number",
                            "minimum": 0.0,
                            "maximum": 1.0,
                            "description": "How important this memory is (0.0-1.0)"
                        }
                    },
                    "required": ["content", "category", "importance"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["memories"],
        "additionalProperties": false
    })
});

pub struct MemoryDistillationPrompt;

impl MemoryDistillationPrompt {
    pub fn config() -> PromptConfig {
        PromptConfig {
            temperature: 0.3,
            max_tokens: 2048,
            response_format: Some(ResponseFormat::structured(
                "memory_extraction".into(),
                MEMORY_EXTRACTION_SCHEMA.clone(),
            )),
            ..PromptConfig::default()
        }
    }

    pub fn messages(
        context_items: &[String],
        existing_memories: &[String],
        goals: &[String],
    ) -> Vec<AiMessage> {
        let locale = FALLBACK_LOCALE;
        let context_text = if context_items.is_empty() {
            t(locale, "prompt-memory-distillation-no-context")
        } else {
            context_items.join("\n---\n")
        };

        let memories_text = if existing_memories.is_empty() {
            t(locale, "prompt-memory-distillation-none")
        } else {
            existing_memories
                .iter()
                .map(|m| format!("- {m}"))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let goals_text = if goals.is_empty() {
            t(locale, "prompt-memory-distillation-none")
        } else {
            goals
                .iter()
                .map(|g| format!("- {g}"))
                .collect::<Vec<_>>()
                .join("\n")
        };

        vec![
            AiMessage::system(t(locale, "prompt-memory-distillation-system")),
            AiMessage::user(t_vars(
                locale,
                "prompt-memory-distillation-user",
                &[
                    ("context_text", context_text),
                    ("memories_text", memories_text),
                    ("goals_text", goals_text),
                ],
            )),
        ]
    }
}

pub struct ConversationMemoryPrompt;

impl ConversationMemoryPrompt {
    pub fn config() -> PromptConfig {
        PromptConfig {
            temperature: 0.3,
            max_tokens: 2048,
            response_format: Some(ResponseFormat::structured(
                "conversation_memory_extraction".into(),
                MEMORY_EXTRACTION_SCHEMA.clone(),
            )),
            ..PromptConfig::default()
        }
    }

    pub fn messages(conversation_turns: &[String], existing_memories: &[String]) -> Vec<AiMessage> {
        let locale = FALLBACK_LOCALE;
        let conversation_text = conversation_turns.join("\n");

        let memories_text = if existing_memories.is_empty() {
            t(locale, "prompt-conversation-memory-no-existing-memories")
        } else {
            existing_memories
                .iter()
                .map(|m| format!("- {m}"))
                .collect::<Vec<_>>()
                .join("\n")
        };

        vec![
            AiMessage::system(t(locale, "prompt-conversation-memory-system")),
            AiMessage::user(t_vars(
                locale,
                "prompt-conversation-memory-user",
                &[
                    ("conversation_text", conversation_text),
                    ("memories_text", memories_text),
                ],
            )),
        ]
    }
}
