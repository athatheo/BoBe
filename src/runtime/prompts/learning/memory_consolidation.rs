//! Memory consolidation prompts for merging short-term into long-term memories.

use serde_json::json;
use std::sync::LazyLock;

use crate::constants::VALID_MEMORY_CATEGORIES;
use crate::i18n::{FALLBACK_LOCALE, t, t_vars};
use crate::llm::types::{AiMessage, ResponseFormat};
use crate::runtime::prompts::base::PromptConfig;

pub(crate) static MEMORY_CONSOLIDATION_SCHEMA: LazyLock<serde_json::Value> = LazyLock::new(|| {
    json!({
        "type": "object",
        "properties": {
            "consolidated_memories": {
                "type": "array",
                "description": "List of consolidated long-term memories",
                "items": {
                    "type": "object",
                    "properties": {
                        "content": {
                            "type": "string",
                            "description": "The consolidated memory content"
                        },
                        "category": {
                            "type": "string",
                            "enum": VALID_MEMORY_CATEGORIES,
                            "description": "Category of the consolidated memory"
                        },
                        "source_indices": {
                            "type": "array",
                            "items": {"type": "integer"},
                            "description": "Indices of source memories that were merged (0-indexed)"
                        }
                    },
                    "required": ["content", "category", "source_indices"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["consolidated_memories"],
        "additionalProperties": false
    })
});

pub(crate) struct MemoryConsolidationPrompt;

impl MemoryConsolidationPrompt {
    pub(crate) fn config() -> PromptConfig {
        PromptConfig {
            temperature: 0.3,
            max_tokens: 1024,
            response_format: Some(ResponseFormat::structured(
                "memory_consolidation".into(),
                MEMORY_CONSOLIDATION_SCHEMA.clone(),
            )),
            ..PromptConfig::default()
        }
    }

    /// Each inner `Vec<String>` is a cluster of similar memories.
    pub(crate) fn messages(
        memory_clusters: &[Vec<String>],
        locale: Option<&str>,
    ) -> Vec<AiMessage> {
        let locale = locale.unwrap_or(FALLBACK_LOCALE);
        use std::fmt::Write;
        let mut clusters_text = String::new();
        let mut global_idx: usize = 0;

        for (i, cluster) in memory_clusters.iter().enumerate() {
            let _ = write!(
                clusters_text,
                "\n{}\n",
                t_vars(
                    locale,
                    "prompt-memory-consolidation-cluster-header",
                    &[("cluster_number", (i + 1).to_string())],
                )
            );
            for memory in cluster {
                let _ = writeln!(
                    clusters_text,
                    "{}",
                    t_vars(
                        locale,
                        "prompt-memory-consolidation-cluster-item",
                        &[
                            ("index", global_idx.to_string()),
                            ("memory", memory.clone())
                        ],
                    )
                );
                global_idx += 1;
            }
        }

        vec![
            AiMessage::system(t(locale, "prompt-memory-consolidation-system")),
            AiMessage::user(t_vars(
                locale,
                "prompt-memory-consolidation-user",
                &[("clusters_text", clusters_text)],
            )),
        ]
    }
}
