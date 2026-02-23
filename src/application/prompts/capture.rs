//! Screen capture analysis prompts.

use crate::application::prompts::base::PromptConfig;
use crate::ports::llm_types::{AiMessage, MessageContent};

/// Allowed category values for validation.
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

/// Prompt for analyzing screenshots with vision LLM — free-text output.
pub struct VisionAnalysisPrompt;

impl VisionAnalysisPrompt {
    const SYSTEM: &str = "\
You are analyzing a screenshot of a user's desktop screen.
Write 1-2 detailed paragraphs describing EXACTLY what is on screen with maximum specificity.

Priorities (most important first):
1. **Exact file names and paths** visible in tabs, title bars, or file trees (e.g. \"capture_learner.py\", \"~/projects/bobe/src/\")
2. **Specific text content** — quote code snippets, error messages, terminal output, or document text you can read
3. **URLs and page titles** from browser tabs or address bars
4. **Application names and window layout** — which apps are open, which is focused, any split/tiled arrangement
5. **General activity** — coding, browsing, writing, debugging, reading docs, etc.

Be concrete: say \"editing capture_learner.py line 385, function _update_visual_memory\" NOT \"writing Python code\".
Say \"browsing GitHub issue #1234: Fix memory pipeline\" NOT \"looking at a website\".
If you can read text on screen, quote it. If you can see file names, list them.";

    const USER: &str = "Describe exactly what is on this screen. Reference specific text and content you can read.";

    pub fn config() -> PromptConfig {
        PromptConfig {
            temperature: 0.4,
            max_tokens: 3000,
            ..PromptConfig::default()
        }
    }

    pub fn messages(image_url: &str) -> Vec<AiMessage> {
        vec![
            AiMessage::system(Self::SYSTEM),
            AiMessage {
                role: "user".into(),
                content: MessageContent::Parts(vec![
                    serde_json::json!({"type": "text", "text": Self::USER}),
                    serde_json::json!({"type": "image_url", "image_url": {"url": image_url}}),
                ]),
                name: None,
                tool_calls: vec![],
                tool_call_id: None,
            },
        ]
    }
}

/// Prompt for updating the visual memory diary with a new observation.
///
/// The LLM receives the existing diary and a new vision-LLM description,
/// then returns the COMPLETE updated diary.
pub struct VisualMemoryConsolidationPrompt;

impl VisualMemoryConsolidationPrompt {
    const SYSTEM: &str = "\
You maintain a visual memory diary — a timestamped log of what the user is doing on their computer.

You will receive:
1. The EXISTING diary (may be empty for the first entry of the day)
2. A NEW observation — a detailed description of the user's current screen from a vision model

Your job: return the COMPLETE updated diary. You may:
- Append a new timestamped entry (most common)
- Merge with the previous entry if it's clearly the same activity (update its summary, keep its timestamp)
- Restructure the last few entries if the new observation clarifies what the user was doing

Format rules:
- Each entry: [HH:MM] Specific summary. Tags: tag1, tag2. Obs: <obs_id>
- Tags: 1-3 lowercase words from {coding, terminal, browsing, documentation, communication, design, media, debugging, reading, writing, configuring, researching, idle, other}
- Obs: must include the provided observation ID exactly
- Preserve the diary header line (e.g. \"# Visual Memory 2026-02-22 PM\") as-is
- Preserve all older entries unchanged — only modify/merge the most recent entry or add new ones

Specificity rules (critical):
- Name the EXACT files, URLs, documents, or pages visible — not just the application.
- Include function/class names, error text, or terminal commands if visible.
- BAD: \"User coding in VS Code.\" → too vague, useless for recall.
- GOOD: \"Editing capture_learner.py — fixing _update_visual_memory, test file open in split.\"
- BAD: \"User browsing the web.\" → says nothing.
- GOOD: \"Reading GitHub PR #42 'Fix memory pipeline' in Firefox, comments tab open.\"
- One sentence per entry, packed with specifics.";

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
        let diary_section = if existing_diary.is_empty() {
            "(empty — this is the first entry of the day)".to_owned()
        } else {
            existing_diary.to_owned()
        };

        let user_text = format!(
            "## Existing diary\n\
             {diary_section}\n\n\
             ## New observation at [{timestamp}]\n\
             {new_observation}\n\n\
             ## Observation ID\n\
             {observation_id}\n\n\
             Return the complete updated diary."
        );

        vec![AiMessage::system(Self::SYSTEM), AiMessage::user(user_text)]
    }
}
