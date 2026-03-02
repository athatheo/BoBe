//! Memory consolidation prompts for merging short-term into long-term memories.

use serde_json::json;
use std::sync::LazyLock;

use crate::llm::types::{AiMessage, ResponseFormat};
use crate::runtime::prompts::base::PromptConfig;

/// JSON Schema for memory consolidation output.
pub static MEMORY_CONSOLIDATION_SCHEMA: LazyLock<serde_json::Value> = LazyLock::new(|| {
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
                            "enum": ["preference", "pattern", "fact", "interest"],
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

/// Prompt for consolidating similar short-term memories into long-term.
pub struct MemoryConsolidationPrompt;

impl MemoryConsolidationPrompt {
    const SYSTEM: &str = "\
You are a memory consolidation system. Your job is to merge similar short-term memories into more general long-term memories.

You will receive clusters of related memories. For each cluster, create a single consolidated memory that:
1. Captures the essential information from all memories in the cluster
2. Is more general and enduring than the individual memories
3. Removes redundancy while preserving important details
4. Uses clear, factual language

Guidelines:
- If memories in a cluster are actually different facts, keep them separate
- If memories represent the same fact with different wording, merge them
- If one memory is more specific than another, prefer the more specific version
- Track which source memories each consolidated memory came from

Example:
Input cluster: [\"User prefers Python\", \"User likes Python for scripting\", \"User uses Python daily\"]
Output: \"User strongly prefers Python, using it daily for scripting\" (merged all 3)";

    pub fn config() -> PromptConfig {
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

    /// Build messages for memory consolidation.
    ///
    /// Each inner `Vec<String>` is a cluster of similar memories.
    pub fn messages(memory_clusters: &[Vec<String>]) -> Vec<AiMessage> {
        use std::fmt::Write;
        let mut clusters_text = String::new();
        let mut global_idx: usize = 0;

        for (i, cluster) in memory_clusters.iter().enumerate() {
            let _ = write!(clusters_text, "\n## Cluster {}\n", i + 1);
            for memory in cluster {
                let _ = writeln!(clusters_text, "[{global_idx}] {memory}");
                global_idx += 1;
            }
        }

        vec![
            AiMessage::system(Self::SYSTEM),
            AiMessage::user(format!(
                "Consolidate the following memory clusters into long-term memories.\n\
                 {clusters_text}\n\
                 For each cluster, create consolidated memories and track which source \
                 indices were merged."
            )),
        ]
    }
}
