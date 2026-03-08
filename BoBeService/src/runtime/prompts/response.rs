use crate::i18n::{FALLBACK_LOCALE, t, t_vars};
use crate::llm::types::AiMessage;
use crate::runtime::prompts::base::{DEFAULT_SOUL, PromptConfig};

pub(crate) struct ProactiveResponsePrompt;

impl ProactiveResponsePrompt {
    pub(crate) fn config() -> PromptConfig {
        PromptConfig {
            temperature: 0.7,
            max_tokens: 2048,
            ..PromptConfig::default()
        }
    }

    pub(crate) fn messages(
        context: &str,
        soul: Option<&str>,
        previous_conversation_summary: Option<&str>,
        current_time: Option<&str>,
        locale: Option<&str>,
    ) -> Vec<AiMessage> {
        let locale = locale.unwrap_or(FALLBACK_LOCALE);
        let language_directive = t(locale, "response-language-directive");
        let system_content = format!(
            "{}\n\n{}\n\n{}",
            soul.unwrap_or(DEFAULT_SOUL),
            t(locale, "response-proactive-system"),
            language_directive,
        );

        let mut user_content_parts: Vec<String> = Vec::new();

        if let Some(time) = current_time {
            user_content_parts.push(t_vars(
                locale,
                "response-proactive-current-time",
                &[("time", time.to_owned())],
            ));
        }

        if let Some(summary) = previous_conversation_summary {
            user_content_parts.push(format!(
                "{}\n{summary}",
                t(locale, "response-proactive-previous-summary")
            ));
        }

        user_content_parts.push(format!(
            "{}\n{context}",
            t(locale, "response-proactive-recent-activity")
        ));

        if previous_conversation_summary.is_some() {
            user_content_parts.push(t(locale, "response-proactive-reference-previous"));
        }

        user_content_parts.push(t(locale, "response-proactive-final-directive"));

        let user_content = user_content_parts.join("\n");

        vec![
            AiMessage::system(system_content),
            AiMessage::user(user_content),
        ]
    }
}

pub(crate) struct UserResponsePrompt;

impl UserResponsePrompt {
    pub(crate) fn config() -> PromptConfig {
        PromptConfig {
            temperature: 0.7,
            max_tokens: 4096,
            ..PromptConfig::default()
        }
    }

    pub(crate) fn messages(
        user_message: &str,
        context: &str,
        conversation_history: Option<&[(&str, &str)]>,
        soul: Option<&str>,
        locale: Option<&str>,
    ) -> Vec<AiMessage> {
        let locale = locale.unwrap_or(FALLBACK_LOCALE);
        let effective_context = if context.is_empty() {
            t(locale, "response-user-no-recent-context")
        } else {
            context.to_owned()
        };

        let language_directive = t(locale, "response-language-directive");
        let system_content = format!(
            "{}\n\n{}\n{}\n\n{}\n\n{}",
            soul.unwrap_or(DEFAULT_SOUL),
            t(locale, "response-user-context-header"),
            effective_context,
            t(locale, "response-user-context-suffix"),
            language_directive,
        );

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
