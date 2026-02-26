//! Memory consolidator — promotes short-term memories to long-term.
//!
//! Clusters similar short-term memories using semantic similarity,
//! merges multi-item clusters via LLM, and promotes singles directly.

use std::sync::Arc;

use arc_swap::ArcSwap;
use serde_json::Value;
use tracing::{debug, info, warn};

use crate::config::Config;
use crate::db::MemoryRepository;
use crate::llm::EmbeddingProvider;
use crate::llm::LlmProvider;
use crate::models::memory::Memory;
use crate::models::types::{MemorySource, MemoryType};
use crate::runtime::prompts::learning::memory_consolidation::MemoryConsolidationPrompt;

/// Valid values for memory categories.
const VALID_CATEGORIES: &[&str] = &["preference", "pattern", "fact", "interest"];

pub struct MemoryConsolidator {
    llm: Arc<dyn LlmProvider>,
    embedding: Arc<dyn EmbeddingProvider>,
    memory_repo: Arc<dyn MemoryRepository>,
    config: Arc<ArcSwap<Config>>,
}

impl MemoryConsolidator {
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

    /// Consolidate short-term memories into long-term.
    pub async fn consolidate(&self, short_term_memories: &[Memory]) -> Vec<Memory> {
        if short_term_memories.is_empty() {
            return Vec::new();
        }

        // Step 1: Cluster memories
        let clusters = self.cluster_memories(short_term_memories);

        debug!(
            input_count = short_term_memories.len(),
            cluster_count = clusters.len(),
            "memory_consolidator.clustered"
        );

        let mut created: Vec<Memory> = Vec::new();

        // Separate single-item and multi-item clusters
        let (singles, multis): (Vec<_>, Vec<_>) = clusters.into_iter().partition(|c| c.len() == 1);

        // Promote single-item clusters directly
        for cluster in &singles {
            if let Some(memory) = cluster.first() {
                match self.promote_single(memory).await {
                    Ok(stored) => created.push(stored),
                    Err(e) => warn!(error = %e, "memory_consolidator.promote_failed"),
                }
            }
        }

        // Merge multi-item clusters via LLM
        if !multis.is_empty() {
            let merged = self.merge_clusters(&multis).await;
            created.extend(merged);
        }

        info!(
            input = short_term_memories.len(),
            single_promoted = singles.len(),
            multi_merged = multis.len(),
            output = created.len(),
            "memory_consolidator.complete"
        );

        created
    }

    fn cluster_memories<'a>(&self, memories: &'a [Memory]) -> Vec<Vec<&'a Memory>> {
        let cfg = self.config.load();
        let cap = cfg.learning_max_memories_per_consolidation as usize;
        let memories: Vec<&Memory> = if memories.len() > cap {
            let mut sorted: Vec<&Memory> = memories.iter().collect();
            sorted.sort_by_key(|m| m.created_at);
            sorted.truncate(cap);
            sorted
        } else {
            memories.iter().collect()
        };

        let threshold = cfg.similarity_clustering_threshold;
        let mut clusters: Vec<Vec<&'a Memory>> = Vec::new();
        let mut centroids: Vec<Vec<f32>> = Vec::new();

        for memory in memories {
            let mem_vec = Self::parse_embedding(memory);

            let Some(ref vec) = mem_vec else {
                clusters.push(vec![memory]);
                continue;
            };

            if centroids.is_empty() {
                clusters.push(vec![memory]);
                centroids.push(vec.clone());
                continue;
            }

            // Find best matching cluster
            let mut best_idx = 0;
            let mut best_score = f64::NEG_INFINITY;
            for (i, centroid) in centroids.iter().enumerate() {
                let sim = cosine_similarity(vec, centroid);
                if sim > best_score {
                    best_score = sim;
                    best_idx = i;
                }
            }

            if best_score >= threshold {
                clusters[best_idx].push(memory);
                centroids[best_idx] = Self::compute_centroid(&clusters[best_idx]);
            } else {
                clusters.push(vec![memory]);
                centroids.push(vec.clone());
            }
        }

        clusters
    }

    fn parse_embedding(memory: &Memory) -> Option<Vec<f32>> {
        memory
            .embedding
            .as_ref()
            .and_then(|e| serde_json::from_str(e).ok())
    }

    fn compute_centroid(memories: &[&Memory]) -> Vec<f32> {
        let mut centroid: Vec<f32> = Vec::new();
        let mut count = 0usize;

        for m in memories {
            if let Some(vec) = Self::parse_embedding(m) {
                if centroid.is_empty() {
                    centroid = vec;
                } else {
                    for (i, &v) in vec.iter().enumerate() {
                        if i < centroid.len() {
                            centroid[i] += v;
                        }
                    }
                }
                count += 1;
            }
        }

        if count > 1 {
            let c = count as f32;
            for v in &mut centroid {
                *v /= c;
            }
        }
        centroid
    }

    async fn promote_single(&self, memory: &Memory) -> Result<Memory, crate::error::AppError> {
        let mut long_term = Memory::new(
            memory.content.clone(),
            MemoryType::LongTerm,
            MemorySource::Observation,
            memory.category.clone(),
        );
        long_term.source = MemorySource::Consolidated;
        long_term.embedding = memory.embedding.clone();

        let stored = self.memory_repo.save(&long_term).await?;
        debug!(
            memory_id = %stored.id,
            source_id = %memory.id,
            "memory_consolidator.promoted"
        );
        Ok(stored)
    }

    async fn merge_clusters(&self, clusters: &[Vec<&Memory>]) -> Vec<Memory> {
        let memory_clusters: Vec<Vec<String>> = clusters
            .iter()
            .map(|cluster| cluster.iter().map(|m| m.content.clone()).collect())
            .collect();

        let messages = MemoryConsolidationPrompt::messages(&memory_clusters);
        let prompt_config = MemoryConsolidationPrompt::config();

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
                warn!(error = %e, "memory_consolidator.llm_error");
                return Vec::new();
            }
            Err(_) => {
                warn!("memory_consolidator.llm_timeout");
                return Vec::new();
            }
        };

        let content = response.message.content.text_or_empty().to_string();
        let consolidated = match serde_json::from_str::<Value>(&content) {
            Ok(data) => data
                .get("consolidated_memories")
                .and_then(|m| m.as_array())
                .cloned()
                .unwrap_or_default(),
            Err(e) => {
                warn!(error = %e, "memory_consolidator.json_parse_error");
                return Vec::new();
            }
        };

        let mut created: Vec<Memory> = Vec::new();

        for item in &consolidated {
            let mem_content = item
                .get("content")
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .trim();
            if mem_content.is_empty() {
                continue;
            }

            let mut category = item
                .get("category")
                .and_then(|c| c.as_str())
                .unwrap_or("fact");
            if !VALID_CATEGORIES.contains(&category) {
                category = "fact";
            }

            let new_embedding = match self.embedding.embed(mem_content).await {
                Ok(e) => e,
                Err(e) => {
                    warn!(error = %e, "memory_consolidator.embed_failed");
                    continue;
                }
            };

            let mut long_term = Memory::new(
                mem_content.to_owned(),
                MemoryType::LongTerm,
                MemorySource::Observation,
                category.to_owned(),
            );
            long_term.source = MemorySource::Consolidated;
            long_term.embedding = Some(serde_json::to_string(&new_embedding).unwrap_or_default());

            match self.memory_repo.save(&long_term).await {
                Ok(stored) => {
                    debug!(memory_id = %stored.id, "memory_consolidator.merged");
                    created.push(stored);
                }
                Err(e) => warn!(error = %e, "memory_consolidator.save_failed"),
            }
        }

        created
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
