//! Goal extraction prompts for detecting user goals from conversations.

use serde_json::json;
use std::sync::LazyLock;

use crate::runtime::prompts::base::PromptConfig;
use crate::llm::types::{AiMessage, ResponseFormat};

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
You are a goal detection system. Your job is to identify goals the user might have based on their conversations and activities.

A goal is something the user wants to achieve or learn. Look for:
- Explicit statements (\"I want to learn X\", \"I need to finish Y\")
- Repeated struggles that suggest a learning goal (\"keeps debugging async code\" → \"Improve async programming skills\")
- Project mentions that suggest deliverables
- Skills they're actively working to improve

Guidelines:
1. Goals should be actionable and achievable
2. Goals should be things the user would recognize as their goals
3. Do NOT infer goals from one-off questions (just curiosity, not a goal)
4. Do NOT create goals about completing specific tasks (too granular)
5. Focus on learning goals, skill development, and project outcomes
6. Be conservative - only infer goals with strong evidence

Return an empty goals array if no clear goals can be inferred.";

    pub fn config() -> PromptConfig {
        PromptConfig {
            temperature: 0.3,
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
