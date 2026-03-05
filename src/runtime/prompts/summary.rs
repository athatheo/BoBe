use crate::i18n::{FALLBACK_LOCALE, t, t_vars};
use crate::llm::types::AiMessage;
use crate::runtime::prompts::base::PromptConfig;

pub(crate) struct ConversationSummaryPrompt;

impl ConversationSummaryPrompt {
    pub(crate) fn config() -> PromptConfig {
        PromptConfig {
            temperature: 0.3,
            max_tokens: 200,
            ..PromptConfig::default()
        }
    }

    pub(crate) fn messages(
        conversation_turns: &[(&str, &str)],
        locale: Option<&str>,
    ) -> Vec<AiMessage> {
        let locale = locale.unwrap_or(FALLBACK_LOCALE);
        let turns_text: String = conversation_turns
            .iter()
            .map(|(role, content)| format!("{}: {content}", role.to_uppercase()))
            .collect::<Vec<_>>()
            .join("\n");

        vec![
            AiMessage::system(t(locale, "prompt-summary-system")),
            AiMessage::user(t_vars(
                locale,
                "prompt-summary-user",
                &[("turns_text", turns_text)],
            )),
        ]
    }
}
