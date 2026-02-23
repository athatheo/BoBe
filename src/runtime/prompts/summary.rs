//! Conversation summary prompt.

use crate::runtime::prompts::base::PromptConfig;
use crate::llm::types::AiMessage;

/// Prompt for generating conversation summaries when closing conversations.
pub struct ConversationSummaryPrompt;

impl ConversationSummaryPrompt {
    const SYSTEM_TEMPLATE: &str = "\
You are summarizing a conversation for future context.
Create a brief summary including:
- Main topics discussed
- Any requests or preferences the user mentioned
- Status of any ongoing matters (resolved/unresolved)

Keep it concise (2-3 sentences max). Focus on information useful for future conversations.";

    pub fn config() -> PromptConfig {
        PromptConfig {
            temperature: 0.3,
            max_tokens: 200,
            ..PromptConfig::default()
        }
    }

    /// Build messages for conversation summary generation.
    ///
    /// `conversation_turns` is a list of `(role, content)` tuples.
    pub fn messages(conversation_turns: &[(&str, &str)]) -> Vec<AiMessage> {
        let turns_text: String = conversation_turns
            .iter()
            .map(|(role, content)| format!("{}: {content}", role.to_uppercase()))
            .collect::<Vec<_>>()
            .join("\n");

        vec![
            AiMessage::system(Self::SYSTEM_TEMPLATE),
            AiMessage::user(format!("Summarize this conversation:\n\n{turns_text}")),
        ]
    }
}
