//! Response generation prompts.

use crate::runtime::prompts::base::{DEFAULT_SOUL, PromptConfig};
use crate::llm::types::AiMessage;

/// Prompt for generating proactive suggestions to the user.
pub struct ProactiveResponsePrompt;

impl ProactiveResponsePrompt {
    const SYSTEM_TEMPLATE: &str = "{soul}\n\n\
        You are offering a proactive suggestion based on what you've observed.\n\
        Be brief, helpful, and specific. Don't be intrusive or obvious.";

    pub fn config() -> PromptConfig {
        PromptConfig {
            temperature: 0.7,
            max_tokens: 2048,
            ..PromptConfig::default()
        }
    }

    pub fn messages(
        context: &str,
        soul: Option<&str>,
        previous_conversation_summary: Option<&str>,
        current_time: Option<&str>,
    ) -> Vec<AiMessage> {
        let system_content = Self::SYSTEM_TEMPLATE.replace("{soul}", soul.unwrap_or(DEFAULT_SOUL));

        let mut user_content_parts: Vec<String> = Vec::new();

        if let Some(time) = current_time {
            user_content_parts.push(format!("Current time: {time}\n"));
        }

        if let Some(summary) = previous_conversation_summary {
            user_content_parts.push(format!("Earlier conversation summary:\n{summary}\n"));
        }

        user_content_parts.push(format!("Recent activity:\n{context}"));

        if previous_conversation_summary.is_some() {
            user_content_parts.push(
                "\nYou may naturally reference the previous conversation if relevant.".into(),
            );
        }

        user_content_parts.push(
            "\nRespond directly with your message (no preamble). \
             Be concise for casual check-ins. \
             For structured reviews or briefings per your soul instructions, \
             be thorough and well-formatted."
                .into(),
        );

        let user_content = user_content_parts.join("\n");

        vec![
            AiMessage::system(system_content),
            AiMessage::user(user_content),
        ]
    }
}

/// Prompt for responding to user messages.
pub struct UserResponsePrompt;

impl UserResponsePrompt {
    const SYSTEM_TEMPLATE: &str = "{soul}\n\n\
        Recent activity context:\n\
        {context}\n\n\
        Use this context to provide relevant, helpful responses.";

    pub fn config() -> PromptConfig {
        PromptConfig {
            temperature: 0.7,
            max_tokens: 4096,
            ..PromptConfig::default()
        }
    }

    /// Build messages for user response.
    ///
    /// `conversation_history` is a list of `(role, content)` tuples for previous turns.
    pub fn messages(
        user_message: &str,
        context: &str,
        conversation_history: Option<&[(&str, &str)]>,
        soul: Option<&str>,
    ) -> Vec<AiMessage> {
        let effective_context = if context.is_empty() {
            "No recent context"
        } else {
            context
        };

        let system_content = Self::SYSTEM_TEMPLATE
            .replace("{soul}", soul.unwrap_or(DEFAULT_SOUL))
            .replace("{context}", effective_context);

        let mut msgs: Vec<AiMessage> = vec![AiMessage::system(system_content)];

        if let Some(history) = conversation_history {
            for &(role, content) in history {
                msgs.push(AiMessage {
                    role: role.to_owned(),
                    content: content.into(),
                    name: None,
                    tool_calls: vec![],
                    tool_call_id: None,
                });
            }
        }

        msgs.push(AiMessage::user(user_message));
        msgs
    }
}
