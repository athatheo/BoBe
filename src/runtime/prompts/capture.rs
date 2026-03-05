use crate::i18n::{FALLBACK_LOCALE, t, t_vars};
use crate::llm::types::{AiMessage, MessageContent};
use crate::runtime::prompts::base::PromptConfig;

pub const ALLOWED_CATEGORIES: &[&str] = &[
    "coding",
    "browsing",
    "communication",
    "documentation",
    "terminal",
    "design",
    "media",
    "other",
];

pub struct VisionAnalysisPrompt;

impl VisionAnalysisPrompt {
    pub fn config() -> PromptConfig {
        PromptConfig {
            temperature: 0.4,
            max_tokens: 3000,
            ..PromptConfig::default()
        }
    }

    pub fn messages(image_url: &str) -> Vec<AiMessage> {
        let locale = FALLBACK_LOCALE;
        vec![
            AiMessage::system(t(locale, "prompt-capture-vision-system")),
            AiMessage {
                role: "user".into(),
                content: MessageContent::Parts(vec![
                    serde_json::json!({"type": "text", "text": t(locale, "prompt-capture-vision-user")}),
                    serde_json::json!({"type": "image_url", "image_url": {"url": image_url}}),
                ]),
                name: None,
                tool_calls: vec![],
                tool_call_id: None,
            },
        ]
    }
}

pub struct VisualMemoryConsolidationPrompt;

impl VisualMemoryConsolidationPrompt {
    pub fn config() -> PromptConfig {
        PromptConfig {
            temperature: 0.3,
            max_tokens: 4096,
            ..PromptConfig::default()
        }
    }

    pub fn messages(
        existing_diary: &str,
        new_observation: &str,
        timestamp: &str,
        observation_id: &str,
    ) -> Vec<AiMessage> {
        let locale = FALLBACK_LOCALE;
        let diary_section = if existing_diary.is_empty() {
            t(locale, "prompt-capture-visual-memory-empty-diary")
        } else {
            existing_diary.to_owned()
        };

        let user_text = t_vars(
            locale,
            "prompt-capture-visual-memory-user",
            &[
                ("diary_section", diary_section),
                ("timestamp", timestamp.to_owned()),
                ("new_observation", new_observation.to_owned()),
                ("observation_id", observation_id.to_owned()),
            ],
        );

        vec![
            AiMessage::system(t(locale, "prompt-capture-visual-memory-system")),
            AiMessage::user(user_text),
        ]
    }
}
