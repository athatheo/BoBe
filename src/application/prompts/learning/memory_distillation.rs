//! Memory distillation prompts for extracting memories from context.

use serde_json::json;
use std::sync::LazyLock;

use crate::application::prompts::base::PromptConfig;
use crate::ports::llm_types::{AiMessage, ResponseFormat};

/// JSON Schema for memory extraction output.
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
                            "enum": ["preference", "pattern", "fact", "interest"],
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

/// Prompt for extracting memories from conversation or context items.
pub struct MemoryDistillationPrompt;

impl MemoryDistillationPrompt {
    const SYSTEM: &str = "\
You are a memory extraction system. Your job is to identify memorable facts about the user from their conversations and activities.

Extract memories that would be useful for personalizing future interactions. Focus on:
- User preferences (tools, languages, workflows they prefer)
- Recurring patterns (how they work, when they work)
- Personal facts (job role, projects, team structure)
- Interests (topics they engage with frequently)

Guidelines:
1. Extract only facts that are explicitly stated or clearly implied
2. Do NOT infer or assume information not present
3. Do NOT extract temporary states (\"user is debugging X\" - too transient)
4. Extract enduring information (\"user prefers Python over JavaScript\")
5. Each memory should be a single, atomic fact
6. Avoid duplicating information across memories
7. Assign importance based on how useful the memory would be long-term

Return an empty memories array if no meaningful memories can be extracted.";

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
        let context_text = if context_items.is_empty() {
            "No context available".to_owned()
        } else {
            context_items.join("\n---\n")
        };

        let memories_text = if existing_memories.is_empty() {
            "None".to_owned()
        } else {
            existing_memories
                .iter()
                .map(|m| format!("- {m}"))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let goals_text = if goals.is_empty() {
            "None".to_owned()
        } else {
            goals
                .iter()
                .map(|g| format!("- {g}"))
                .collect::<Vec<_>>()
                .join("\n")
        };

        vec![
            AiMessage::system(Self::SYSTEM),
            AiMessage::user(format!(
                "Extract memorable facts about the user from the following context.\n\n\
                 ## Recent Context\n\
                 {context_text}\n\n\
                 ## Already Known (do not duplicate)\n\
                 {memories_text}\n\n\
                 ## User's Goals (for context)\n\
                 {goals_text}\n\n\
                 Extract any new memories that would help personalize future interactions."
            )),
        ]
    }
}

/// Prompt for extracting memories from a closed conversation.
pub struct ConversationMemoryPrompt;

impl ConversationMemoryPrompt {
    const SYSTEM: &str = "\
You are a memory extraction system analyzing a completed conversation between a user and an AI assistant.

Extract lasting memories about the user that would improve future conversations. Focus on:
- What the user was trying to accomplish (if successful, they may do it again)
- How they prefer to work (communication style, detail level)
- Technical preferences revealed (languages, frameworks, tools)
- Personal context mentioned (role, team, project names)

DO NOT extract:
- The specific task they were working on (too transient)
- Things the AI taught them (they now know it)
- Frustrations or temporary states
- Information that's only relevant to this conversation

Return an empty memories array if the conversation doesn't reveal lasting insights about the user.";

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
        let conversation_text = conversation_turns.join("\n");

        let memories_text = if existing_memories.is_empty() {
            "None".to_owned()
        } else {
            existing_memories
                .iter()
                .map(|m| format!("- {m}"))
                .collect::<Vec<_>>()
                .join("\n")
        };

        vec![
            AiMessage::system(Self::SYSTEM),
            AiMessage::user(format!(
                "Extract lasting memories from this conversation.\n\n\
                 ## Conversation\n\
                 {conversation_text}\n\n\
                 ## Already Known (do not duplicate)\n\
                 {memories_text}\n\n\
                 What lasting facts about the user does this conversation reveal?"
            )),
        ]
    }
}
