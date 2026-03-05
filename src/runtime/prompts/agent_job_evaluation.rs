use crate::i18n::{FALLBACK_LOCALE, t, t_vars};
use crate::llm::types::AiMessage;
use crate::runtime::prompts::base::PromptConfig;

pub struct AgentJobEvaluationPrompt;

impl AgentJobEvaluationPrompt {
    pub fn config() -> PromptConfig {
        PromptConfig {
            temperature: 0.2,
            max_tokens: 50,
            ..PromptConfig::default()
        }
    }

    pub fn messages(
        user_intent: &str,
        result_summary: &str,
        error_message: Option<&str>,
        continuation_count: u32,
    ) -> Vec<AiMessage> {
        let locale = FALLBACK_LOCALE;
        let mut user_parts = vec![
            t_vars(
                locale,
                "prompt-agent-job-evaluation-original-task",
                &[("user_intent", user_intent.to_owned())],
            ),
            t_vars(
                locale,
                "prompt-agent-job-evaluation-agent-result",
                &[(
                    "result_summary",
                    if result_summary.is_empty() {
                        t(locale, "prompt-agent-job-evaluation-no-summary")
                    } else {
                        result_summary.to_owned()
                    },
                )],
            ),
        ];

        if let Some(err) = error_message {
            user_parts.push(t_vars(
                locale,
                "prompt-agent-job-evaluation-agent-error",
                &[("error", err.to_owned())],
            ));
        }

        if continuation_count > 0 {
            user_parts.push(t_vars(
                locale,
                "prompt-agent-job-evaluation-continuation-count",
                &[("count", continuation_count.to_string())],
            ));
        }

        user_parts.push(t(locale, "prompt-agent-job-evaluation-final-directive"));

        vec![
            AiMessage::system(t(locale, "prompt-agent-job-evaluation-system")),
            AiMessage::user(user_parts.join("\n")),
        ]
    }
}
