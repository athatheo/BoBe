//! Memory learner — extracts memories from observations and conversations.
//!
//! Uses LLM to distill observations into memorable facts, then deduplicates
//! against existing memories using semantic similarity + LLM decision.

use std::sync::Arc;

use arc_swap::ArcSwap;
use serde_json::Value;
use tracing::{info, warn};

use crate::config::Config;
use crate::db::MemoryRepository;
use crate::llm::EmbeddingProvider;
use crate::llm::LlmProvider;
use crate::models::goal::Goal;
use crate::models::memory::Memory;
use crate::models::observation::Observation;
use crate::models::types::{MemorySource, MemoryType};
use crate::runtime::prompts::learning::deduplication_decision::MemoryDeduplicationPrompt;
use crate::runtime::prompts::learning::memory_distillation::{
    ConversationMemoryPrompt, MemoryDistillationPrompt,
};
use crate::util::similarity::cosine_similarity;

/// Valid values for memory categories.
const VALID_CATEGORIES: &[&str] = &["preference", "pattern", "fact", "interest"];

/// Threshold for initial semantic search.
const SIMILARITY_SEARCH_THRESHOLD: f64 = 0.5;
const SPECULATIVE_MARKERS: &[&str] = &[
    "probably",
    "maybe",
    "might",
    "possibly",
    "seems",
    "appears",
    "likely",
    "i think",
    "guess",
    "could be",
    "apparently",
];
const RECURRING_PATTERN_MARKERS: &[&str] = &[
    "often",
    "usually",
    "regularly",
    "frequently",
    "repeatedly",
    "habit",
    "routine",
    "recurring",
    "tends to",
    "every ",
    "each ",
    "daily",
    "weekly",
    "always",
];
const TRANSIENT_PATTERN_MARKERS: &[&str] = &[
    "today",
    "currently",
    "right now",
    "at the moment",
    "this time",
    "just now",
    "for now",
];

pub struct MemoryLearner {
    llm: Arc<dyn LlmProvider>,
    embedding: Arc<dyn EmbeddingProvider>,
    memory_repo: Arc<dyn MemoryRepository>,
    config: Arc<ArcSwap<Config>>,
}

impl MemoryLearner {
    pub fn new(
        llm: Arc<dyn LlmProvider>,
        embedding: Arc<dyn EmbeddingProvider>,
        memory_repo: Arc<dyn MemoryRepository>,
        config: Arc<ArcSwap<Config>>,
    ) -> Self {
        Self {
            llm,
            embedding,
            memory_repo,
            config,
        }
    }

    /// Extract memories from accumulated observations.
    pub async fn distill_from_observations(
        &self,
        observations: &[Observation],
        existing_memories: &[Memory],
        goals: &[Goal],
    ) -> Vec<Memory> {
        if observations.is_empty() {
            return Vec::new();
        }

        let context_strings: Vec<String> = observations
            .iter()
            .map(|obs| format!("[{}] {}", obs.category, obs.content))
            .collect();
        let memory_strings: Vec<String> = existing_memories
            .iter()
            .map(|m| m.content.clone())
            .collect();
        let goal_strings: Vec<String> = goals.iter().map(|g| g.content.clone()).collect();

        let messages =
            MemoryDistillationPrompt::messages(&context_strings, &memory_strings, &goal_strings);
        let prompt_config = MemoryDistillationPrompt::config();

        let response = match tokio::time::timeout(
            std::time::Duration::from_secs(240),
            self.llm.complete(
                &messages,
                None,
                prompt_config.response_format.as_ref(),
                prompt_config.temperature,
                prompt_config.max_tokens,
            ),
        )
        .await
        {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => {
                warn!(error = %e, "memory_learner.llm_error");
                return Vec::new();
            }
            Err(_) => {
                warn!("memory_learner.llm_timeout");
                return Vec::new();
            }
        };

        let content = response.message.content.text_or_empty().to_string();
        if content.trim().is_empty() {
            return Vec::new();
        }

        let raw_memories = match serde_json::from_str::<Value>(&content) {
            Ok(data) => data
                .get("memories")
                .and_then(|m| m.as_array())
                .cloned()
                .unwrap_or_default(),
            Err(e) => {
                warn!(error = %e, "memory_learner.json_parse_error");
                return Vec::new();
            }
        };

        self.deduplicate_and_store(&raw_memories, existing_memories)
            .await
    }

    /// Extract memories from a closed conversation.
    pub async fn distill_from_conversation(
        &self,
        conversation_turns: &[(String, String)],
        existing_memories: &[Memory],
    ) -> Vec<Memory> {
        if conversation_turns.is_empty() {
            return Vec::new();
        }

        let turn_strings: Vec<String> = conversation_turns
            .iter()
            .map(|(role, content)| format!("{role}: {content}"))
            .collect();
        let memory_strings: Vec<String> = existing_memories
            .iter()
            .map(|m| m.content.clone())
            .collect();

        let messages = ConversationMemoryPrompt::messages(&turn_strings, &memory_strings);
        let prompt_config = ConversationMemoryPrompt::config();

        let response = match tokio::time::timeout(
            std::time::Duration::from_secs(240),
            self.llm.complete(
                &messages,
                None,
                prompt_config.response_format.as_ref(),
                prompt_config.temperature,
                prompt_config.max_tokens,
            ),
        )
        .await
        {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => {
                warn!(error = %e, "memory_learner.llm_error");
                return Vec::new();
            }
            Err(_) => {
                warn!("memory_learner.llm_timeout");
                return Vec::new();
            }
        };

        let content = response.message.content.text_or_empty().to_string();
        if content.trim().is_empty() {
            return Vec::new();
        }

        let raw_memories = match serde_json::from_str::<Value>(&content) {
            Ok(data) => data
                .get("memories")
                .and_then(|m| m.as_array())
                .cloned()
                .unwrap_or_default(),
            Err(e) => {
                warn!(error = %e, "memory_learner.json_parse_error");
                return Vec::new();
            }
        };

        self.deduplicate_and_store(&raw_memories, existing_memories)
            .await
    }

    async fn deduplicate_and_store(
        &self,
        raw_memories: &[Value],
        existing_memories: &[Memory],
    ) -> Vec<Memory> {
        let cfg = self.config.load();

        // Build existing embeddings for dedup
        let existing_embeddings: Vec<(&Memory, Vec<f32>)> = existing_memories
            .iter()
            .filter_map(|mem| {
                mem.embedding
                    .as_ref()
                    .and_then(|e| serde_json::from_str::<Vec<f32>>(e).ok().map(|v| (mem, v)))
            })
            .collect();

        let mut created: Vec<Memory> = Vec::new();
        let max_memories = cfg.learning.max_memories_per_cycle as usize;

        for raw in raw_memories {
            if created.len() >= max_memories {
                break;
            }

            let Some((content, category)) = Self::sanitize_candidate_memory(raw) else {
                continue;
            };

            // Generate embedding
            let new_embedding = match self.embedding.embed(&content).await {
                Ok(e) => e,
                Err(e) => {
                    warn!(error = %e, "memory_learner.embedding_failed");
                    continue;
                }
            };

            // Check batch duplicates
            let is_batch_dup = created
                .iter()
                .any(|m| m.content.trim().eq_ignore_ascii_case(content.as_str()));
            if is_batch_dup {
                continue;
            }

            // LLM-based deduplication
            if !self
                .should_create_memory(&content, &category, &new_embedding, &existing_embeddings)
                .await
            {
                continue;
            }

            // Create and store
            let mut memory = Memory::new(
                content.clone(),
                MemoryType::ShortTerm,
                MemorySource::Observation,
                category.clone(),
            );
            memory.embedding = match serde_json::to_string(&new_embedding) {
                Ok(serialized) => Some(serialized),
                Err(e) => {
                    warn!(error = %e, "memory_learner.embedding_serialize_failed");
                    continue;
                }
            };

            match self.memory_repo.save(&memory).await {
                Ok(stored) => {
                    info!(
                        item_id = %stored.id,
                        category = %category,
                        "memory_learner.stored"
                    );
                    created.push(stored);
                }
                Err(e) => {
                    warn!(error = %e, "memory_learner.save_failed");
                }
            }
        }

        created
    }

    fn sanitize_candidate_memory(raw: &Value) -> Option<(String, String)> {
        let content_raw = raw.get("content").and_then(|c| c.as_str()).unwrap_or("");
        let content = Self::normalize_memory_content(content_raw);
        if content.is_empty() {
            return None;
        }

        if Self::contains_speculative_language(&content) {
            warn!(
                content_preview = %Self::preview(&content),
                "memory_learner.speculative_memory_rejected"
            );
            return None;
        }

        let mut category = raw
            .get("category")
            .and_then(|c| c.as_str())
            .unwrap_or("fact")
            .to_ascii_lowercase();
        if !VALID_CATEGORIES.contains(&category.as_str()) {
            category = "fact".into();
        }

        if category == "pattern" && !Self::is_high_signal_pattern(&content) {
            warn!(
                content_preview = %Self::preview(&content),
                "memory_learner.low_signal_pattern_rejected"
            );
            return None;
        }

        Some((content, category))
    }

    fn normalize_memory_content(content: &str) -> String {
        content
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .trim_matches('"')
            .trim()
            .to_owned()
    }

    fn contains_speculative_language(content: &str) -> bool {
        let lowered = content.to_ascii_lowercase();
        SPECULATIVE_MARKERS
            .iter()
            .any(|marker| lowered.contains(marker))
    }

    fn is_high_signal_pattern(content: &str) -> bool {
        let lowered = content.to_ascii_lowercase();
        let has_recurring_signal = RECURRING_PATTERN_MARKERS
            .iter()
            .any(|marker| lowered.contains(marker));
        let has_transient_signal = TRANSIENT_PATTERN_MARKERS
            .iter()
            .any(|marker| lowered.contains(marker));

        has_recurring_signal && !has_transient_signal
    }

    fn preview(content: &str) -> String {
        const MAX_CHARS: usize = 120;
        let mut out = String::new();
        for ch in content.chars().take(MAX_CHARS) {
            out.push(ch);
        }
        out
    }

    async fn should_create_memory(
        &self,
        content: &str,
        category: &str,
        embedding: &[f32],
        existing_embeddings: &[(&Memory, Vec<f32>)],
    ) -> bool {
        // Find similar existing memories
        let mut similar: Vec<(&uuid::Uuid, &str, &str, f64)> = Vec::new();
        for (mem, existing_vec) in existing_embeddings {
            let sim = cosine_similarity(embedding, existing_vec);
            if sim >= SIMILARITY_SEARCH_THRESHOLD {
                similar.push((&mem.id, &mem.content, &mem.category, sim));
            }
        }
        similar.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal));
        similar.truncate(5);

        if similar.is_empty() {
            return true;
        }

        let existing_data: Vec<(String, String, String)> = similar
            .iter()
            .map(|(id, content, cat, _)| (id.to_string(), content.to_string(), cat.to_string()))
            .collect();

        let messages = MemoryDeduplicationPrompt::messages(content, category, &existing_data);
        let prompt_config = MemoryDeduplicationPrompt::config();

        let response = match tokio::time::timeout(
            std::time::Duration::from_secs(240),
            self.llm.complete(
                &messages,
                None,
                prompt_config.response_format.as_ref(),
                prompt_config.temperature,
                prompt_config.max_tokens,
            ),
        )
        .await
        {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => {
                warn!(error = %e, "memory_learner.dedup_llm_error");
                return true;
            }
            Err(_) => {
                warn!("memory_learner.dedup_llm_timeout");
                return true;
            }
        };

        let resp_content = response.message.content.text_or_empty().to_string();
        match serde_json::from_str::<Value>(&resp_content) {
            Ok(data) => {
                let decision = data
                    .get("decision")
                    .and_then(|d| d.as_str())
                    .unwrap_or("CREATE");
                match decision.to_ascii_uppercase().as_str() {
                    "CREATE" => true,
                    "SKIP" => false,
                    _ => {
                        warn!(decision = %decision, "memory_learner.dedup_unknown_decision");
                        true
                    }
                }
            }
            Err(e) => {
                warn!(error = %e, "memory_learner.dedup_json_parse_error");
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::MemoryLearner;

    #[test]
    fn rejects_speculative_claims() {
        let raw = json!({
            "content": "User probably prefers working late nights.",
            "category": "pattern"
        });

        assert!(MemoryLearner::sanitize_candidate_memory(&raw).is_none());
    }

    #[test]
    fn rejects_low_signal_patterns() {
        let raw = json!({
            "content": "User worked on Rust today.",
            "category": "pattern"
        });

        assert!(MemoryLearner::sanitize_candidate_memory(&raw).is_none());
    }

    #[test]
    fn keeps_high_signal_patterns() {
        let raw = json!({
            "content": "User usually reviews CI failures every morning before standup.",
            "category": "pattern"
        });

        let sanitized = MemoryLearner::sanitize_candidate_memory(&raw).expect("expected memory");
        assert_eq!(sanitized.1, "pattern");
    }

    #[test]
    fn normalizes_content_and_category() {
        let raw = json!({
            "content": "  \"User prefers concise PR reviews\"  ",
            "category": "unsupported"
        });

        let sanitized = MemoryLearner::sanitize_candidate_memory(&raw).expect("expected memory");
        assert_eq!(sanitized.0, "User prefers concise PR reviews");
        assert_eq!(sanitized.1, "fact");
    }
}
