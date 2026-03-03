//! Goal extraction prompts for detecting user goals from conversations.

use serde_json::json;
use std::sync::LazyLock;

use crate::llm::types::{AiMessage, ResponseFormat};
use crate::runtime::prompts::base::PromptConfig;

/// JSON Schema for goal extraction output.
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

/// Prompt for detecting goals from user conversations.
pub struct GoalExtractionPrompt;

impl GoalExtractionPrompt {
    const SYSTEM: &str = "\
You are a goal detection system. Your DEFAULT response is {\"goals\": []}. Goal creation is RARE.

Only create a goal when you see ONE of these strong signals:
1. EXPLICIT USER STATEMENT: The user clearly says \"I want to...\", \"I need to...\", \"My goal is...\" — an unambiguous declaration of intent.
2. MULTI-SESSION COMMITMENT: The user has brought up the same objective across multiple conversations, showing sustained commitment (not just one mention).

Do NOT create goals for:
- Passing mentions of topics or interests
- One-off questions or curiosity
- Single conversations about a topic (even long ones)
- Vague aspirations without clear intent (\"it would be nice to...\")
- Specific tasks or micro-tasks (too granular)
- Skills the user is already competent at

Guidelines:
1. Goals should be actionable and achievable
2. Goals should be things the user would explicitly recognize as their goals
3. When in doubt, return empty — the cost of a spurious goal is much higher than missing one
4. Focus only on goals with overwhelming evidence of user intent

Return an empty goals array if no clear goals can be inferred (this should be most of the time).";

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
        let conversation_text = conversation_turns.join("\n");

        let goals_text = if existing_goals.is_empty() {
            "None".to_owned()
        } else {
            existing_goals
                .iter()
                .map(|g| format!("- {g}"))
                .collect::<Vec<_>>()
                .join("\n")
        };

        vec![
            AiMessage::system(Self::SYSTEM),
            AiMessage::user(format!(
                "Identify any goals the user might have based on this conversation.\n\n\
                 ## Conversation\n\
                 {conversation_text}\n\n\
                 ## Already Known Goals (do not duplicate)\n\
                 {goals_text}\n\n\
                 What new goals can you infer from this conversation?"
            )),
        ]
    }
}
