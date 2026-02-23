//! Memory learner — extracts memories from observations and conversations.
//!
//! Uses LLM to distill observations into memorable facts, then deduplicates
//! against existing memories using semantic similarity + LLM decision.

use std::sync::Arc;

use serde_json::Value;
use tracing::{info, warn};

use crate::application::learning::config::LearningConfig;
use crate::application::prompts::learning::deduplication_decision::MemoryDeduplicationPrompt;
use crate::application::prompts::learning::memory_distillation::{
    ConversationMemoryPrompt, MemoryDistillationPrompt,
};
use crate::domain::goal::Goal;
use crate::domain::memory::Memory;
use crate::domain::observation::Observation;
use crate::domain::types::{MemorySource, MemoryType};
use crate::ports::embedding::EmbeddingProvider;
use crate::ports::llm::LlmProvider;
use crate::ports::repos::memory_repo::MemoryRepository;

/// Valid values for memory categories.
const VALID_CATEGORIES: &[&str] = &["preference", "pattern", "fact", "interest"];

/// Threshold for initial semantic search.
const SIMILARITY_SEARCH_THRESHOLD: f64 = 0.5;

pub struct MemoryLearner {
    llm: Arc<dyn LlmProvider>,
    embedding: Arc<dyn EmbeddingProvider>,
    memory_repo: Arc<dyn MemoryRepository>,
    config: LearningConfig,
}

impl MemoryLearner {
    pub fn new(
        llm: Arc<dyn LlmProvider>,
        embedding: Arc<dyn EmbeddingProvider>,
        memory_repo: Arc<dyn MemoryRepository>,
        config: LearningConfig,
    ) -> Self {
        Self {
            llm,
            embedding,
            memory_repo,
            config,
        }
    }

    pub fn update_config(&mut self, config: LearningConfig) {
        self.config = config;
    }

    pub fn update_llm(&mut self, llm: Arc<dyn LlmProvider>) {
        self.llm = llm;
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
        let config = MemoryDistillationPrompt::config();

        let response = match tokio::time::timeout(
            std::time::Duration::from_secs(240),
            self.llm.complete(
                &messages,
                None,
                config.response_format.as_ref(),
                config.temperature,
                config.max_tokens,
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
        let config = ConversationMemoryPrompt::config();

        let response = match tokio::time::timeout(
            std::time::Duration::from_secs(240),
            self.llm.complete(
                &messages,
                None,
                config.response_format.as_ref(),
                config.temperature,
                config.max_tokens,
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
        let max_memories = self.config.max_memories_per_cycle as usize;

        for raw in raw_memories {
            if created.len() >= max_memories {
                break;
            }

            let content = raw
                .get("content")
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .trim();
            if content.is_empty() {
                continue;
            }

            let mut category = raw
                .get("category")
                .and_then(|c| c.as_str())
                .unwrap_or("fact");
            if !VALID_CATEGORIES.contains(&category) {
                category = "fact";
            }

            // Generate embedding
            let new_embedding = match self.embedding.embed(content).await {
                Ok(e) => e,
                Err(e) => {
                    warn!(error = %e, "memory_learner.embedding_failed");
                    continue;
                }
            };

            // Check batch duplicates
            let is_batch_dup = created
                .iter()
                .any(|m| m.content.to_lowercase().trim() == content.to_lowercase().trim());
            if is_batch_dup {
                continue;
            }

            // LLM-based deduplication
            if !self
                .should_create_memory(content, category, &new_embedding, &existing_embeddings)
                .await
            {
                continue;
            }

            // Create and store
            let mut memory = Memory::new(
                content.to_owned(),
                MemoryType::ShortTerm,
                MemorySource::Observation,
                category.to_owned(),
            );
            memory.embedding = Some(serde_json::to_string(&new_embedding).unwrap_or_default());

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
        let config = MemoryDeduplicationPrompt::config();

        let response = match tokio::time::timeout(
            std::time::Duration::from_secs(240),
            self.llm.complete(
                &messages,
                None,
                config.response_format.as_ref(),
                config.temperature,
                config.max_tokens,
            ),
        )
        .await
        {
            Ok(Ok(r)) => r,
            _ => return true, // Default to create on error
        };

        let resp_content = response.message.content.text_or_empty().to_string();
        match serde_json::from_str::<Value>(&resp_content) {
            Ok(data) => {
                let decision = data
                    .get("decision")
                    .and_then(|d| d.as_str())
                    .unwrap_or("CREATE");
                decision.to_uppercase() == "CREATE"
            }
            Err(_) => true,
        }
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f64 = a
        .iter()
        .zip(b.iter())
        .map(|(x, y)| (*x as f64) * (*y as f64))
        .sum();
    let norm_a: f64 = a.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    let denom = norm_a * norm_b;
    if denom < 1e-8 { 0.0 } else { dot / denom }
}
