use crate::i18n::{FALLBACK_LOCALE, t, t_vars};
use crate::llm::types::{AiMessage, ResponseFormat};
use crate::runtime::prompts::base::PromptConfig;

pub(crate) struct GoalDeduplicationPrompt;

impl GoalDeduplicationPrompt {
    pub(crate) fn config() -> PromptConfig {
        PromptConfig {
            temperature: 0.0,
            max_tokens: 300,
            response_format: Some(ResponseFormat::json()),
            ..PromptConfig::default()
        }
    }

    fn system_message(locale: &str) -> String {
        t(locale, "prompt-goal-dedup-system")
    }

    pub(crate) fn messages(
        candidate_content: &str,
        existing_goals: &[(String, String, String)],
        locale: Option<&str>,
    ) -> Vec<AiMessage> {
        let locale = locale.unwrap_or(FALLBACK_LOCALE);
        let user_content = if existing_goals.is_empty() {
            t_vars(
                locale,
                "prompt-goal-dedup-user-no-existing",
                &[("candidate_content", candidate_content.to_owned())],
            )
        } else {
            let existing_list: String = existing_goals
                .iter()
                .map(|(id, content, priority)| {
                    t_vars(
                        locale,
                        "prompt-goal-dedup-existing-item",
                        &[
                            ("id", id.clone()),
                            ("priority", priority.clone()),
                            ("content", content.clone()),
                        ],
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");

            t_vars(
                locale,
                "prompt-goal-dedup-user-with-existing",
                &[
                    ("candidate_content", candidate_content.to_owned()),
                    ("existing_list", existing_list),
                ],
            )
        };

        vec![
            AiMessage::system(Self::system_message(locale)),
            AiMessage::user(user_content),
        ]
    }
}

pub(crate) struct MemoryDeduplicationPrompt;

impl MemoryDeduplicationPrompt {
    pub(crate) fn config() -> PromptConfig {
        PromptConfig {
            temperature: 0.0,
            max_tokens: 300,
            response_format: Some(ResponseFormat::json()),
            ..PromptConfig::default()
        }
    }

    fn system_message(locale: &str) -> String {
        t(locale, "prompt-memory-dedup-system")
    }

    pub(crate) fn messages(
        candidate_content: &str,
        candidate_category: &str,
        existing_memories: &[(String, String, String)],
        locale: Option<&str>,
    ) -> Vec<AiMessage> {
        let locale = locale.unwrap_or(FALLBACK_LOCALE);
        let user_content = if existing_memories.is_empty() {
            t_vars(
                locale,
                "prompt-memory-dedup-user-no-existing",
                &[
                    ("candidate_category", candidate_category.to_owned()),
                    ("candidate_content", candidate_content.to_owned()),
                ],
            )
        } else {
            let existing_list: String = existing_memories
                .iter()
                .map(|(id, content, cat)| {
                    t_vars(
                        locale,
                        "prompt-memory-dedup-existing-item",
                        &[
                            ("id", id.clone()),
                            ("category", cat.clone()),
                            ("content", content.clone()),
                        ],
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");

            t_vars(
                locale,
                "prompt-memory-dedup-user-with-existing",
                &[
                    ("candidate_category", candidate_category.to_owned()),
                    ("candidate_content", candidate_content.to_owned()),
                    ("existing_list", existing_list),
                ],
            )
        };

        vec![
            AiMessage::system(Self::system_message(locale)),
            AiMessage::user(user_content),
        ]
    }
}
