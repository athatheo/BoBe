//! ContextAssembler — single source of truth for LLM context retrieval.
//!
//! Coordinates retrieval from multiple sources (memories, goals, souls, observations)
//! to build context for LLM prompts. Read-only: create/update/delete goes through
//! specific services or repositories.

use std::sync::Arc;

use tracing::{debug, error, warn};

use crate::domain::goal::Goal;
use crate::domain::memory::Memory;
use crate::domain::observation::Observation;
use crate::domain::soul::Soul;
use crate::ports::embedding::EmbeddingProvider;
use crate::ports::repos::goal_repo::GoalRepository;
use crate::ports::repos::memory_repo::MemoryRepository;
use crate::ports::repos::observation_repo::ObservationRepository;
use crate::ports::repos::soul_repo::SoulRepository;

use super::soul_service::SoulService;

/// Container for assembled context items from all sources.
#[derive(Debug, Clone, Default)]
pub struct AssembledContext {
    pub souls: Vec<Soul>,
    pub goals: Vec<Goal>,
    pub memories: Vec<Memory>,
    pub observations: Vec<Observation>,
}

impl AssembledContext {
    /// Format assembled context for LLM prompt injection.
    /// Only includes sections that have content.
    pub fn to_prompt_sections(&self) -> std::collections::HashMap<String, String> {
        let mut sections = std::collections::HashMap::new();

        if !self.souls.is_empty() {
            let content: Vec<String> = self.souls.iter().map(|s| s.content.clone()).collect();
            sections.insert("personality".into(), content.join("\n\n"));
        }

        if !self.goals.is_empty() {
            let lines: Vec<String> = self
                .goals
                .iter()
                .map(|g| {
                    let priority = if g.priority.is_empty() { "medium" } else { &g.priority };
                    format!("- {} (priority: {})", g.content, priority)
                })
                .collect();
            sections.insert("current_goals".into(), lines.join("\n"));
        }

        if !self.memories.is_empty() {
            let lines: Vec<String> =
                self.memories.iter().map(|m| format!("- {}", m.content)).collect();
            sections.insert("relevant_memories".into(), lines.join("\n"));
        }

        if !self.observations.is_empty() {
            let lines: Vec<String> = self
                .observations
                .iter()
                .map(|o| {
                    // Check metadata for pre-computed summary
                    let summary = o.metadata.as_ref()
                        .and_then(|m| serde_json::from_str::<serde_json::Value>(m).ok())
                        .and_then(|v| v.get("summary").and_then(|s| s.as_str()).map(|s| s.to_string()))
                        .unwrap_or_else(|| {
                            if o.content.len() > 100 { o.content[..100].to_string() } else { o.content.clone() }
                        });
                    let category = if o.category.is_empty() { "general" } else { &o.category };
                    format!("- [{}] {}", category, summary)
                })
                .collect();
            sections.insert("recent_context".into(), lines.join("\n"));
        }

        sections
    }

    /// Format assembled context into a single context string and optional personality.
    pub fn to_context_string(&self) -> (String, Option<String>) {
        let sections = self.to_prompt_sections();
        let mut context = sections.get("recent_context").cloned().unwrap_or_default();
        let personality = sections.get("personality").cloned();

        if let Some(goals) = sections.get("current_goals") {
            context = format!("User goals:\n{goals}\n\n{context}");
        }
        if let Some(memories) = sections.get("relevant_memories") {
            context = format!("Relevant memories:\n{memories}\n\n{context}");
        }
        (context, personality)
    }

    pub fn is_empty(&self) -> bool {
        self.souls.is_empty()
            && self.goals.is_empty()
            && self.memories.is_empty()
            && self.observations.is_empty()
    }
}

/// Options for building context.
pub struct BuildContextOptions {
    pub include_memories: bool,
    pub include_goals: bool,
    pub include_souls: bool,
    pub include_observations: bool,
    pub memory_limit: i64,
    pub observation_limit: i64,
    pub memory_min_score: f64,
}

impl Default for BuildContextOptions {
    fn default() -> Self {
        Self {
            include_memories: true,
            include_goals: true,
            include_souls: true,
            include_observations: false,
            memory_limit: 5,
            observation_limit: 3,
            memory_min_score: 0.6,
        }
    }
}

/// Assembles context from multiple sources for LLM prompts.
pub struct ContextAssembler {
    soul_repo: Arc<dyn SoulRepository>,
    goal_repo: Arc<dyn GoalRepository>,
    memory_repo: Arc<dyn MemoryRepository>,
    observation_repo: Arc<dyn ObservationRepository>,
    embedding: Arc<dyn EmbeddingProvider>,
    soul_service: Option<Arc<SoulService>>,
}

impl ContextAssembler {
    pub fn new(
        soul_repo: Arc<dyn SoulRepository>,
        goal_repo: Arc<dyn GoalRepository>,
        memory_repo: Arc<dyn MemoryRepository>,
        observation_repo: Arc<dyn ObservationRepository>,
        embedding: Arc<dyn EmbeddingProvider>,
        soul_service: Option<Arc<SoulService>>,
    ) -> Self {
        Self {
            soul_repo,
            goal_repo,
            memory_repo,
            observation_repo,
            embedding,
            soul_service,
        }
    }

    /// Build complete context for an LLM prompt.
    pub async fn build_context(
        &self,
        query: &str,
        opts: BuildContextOptions,
    ) -> AssembledContext {
        let mut results = AssembledContext::default();

        // Generate embedding once, reuse for all semantic searches
        let embedding = if !query.is_empty()
            && (opts.include_memories || opts.include_observations)
        {
            match self.embedding.embed(query).await {
                Ok(v) => Some(v),
                Err(e) => {
                    error!(error = %e, "context_assembler.embedding_failed");
                    None
                }
            }
        } else {
            None
        };

        if opts.include_souls {
            match self.soul_repo.find_enabled().await {
                Ok(souls) => results.souls = souls,
                Err(e) => error!(error = %e, "context_assembler.souls_failed"),
            }
        }

        if opts.include_goals {
            match self.goal_repo.find_active(true).await {
                Ok(goals) => results.goals = goals,
                Err(e) => error!(error = %e, "context_assembler.goals_failed"),
            }
        }

        if opts.include_memories
            && let Some(ref emb) = embedding {
                match self
                    .memory_repo
                    .find_similar(emb, opts.memory_limit, true, opts.memory_min_score)
                    .await
                {
                    Ok(pairs) => {
                        results.memories = pairs.into_iter().map(|(m, _)| m).collect();
                    }
                    Err(e) => error!(error = %e, "context_assembler.memories_failed"),
                }
            }

        if opts.include_observations {
            if let Some(ref emb) = embedding {
                match self.observation_repo.find_similar(emb, opts.observation_limit).await {
                    Ok(pairs) => {
                        results.observations = pairs.into_iter().map(|(o, _)| o).collect();
                    }
                    Err(e) => error!(error = %e, "context_assembler.observations_failed"),
                }
            } else {
                match self.observation_repo.find_recent(30).await {
                    Ok(mut obs) => {
                        obs.truncate(opts.observation_limit as usize);
                        results.observations = obs;
                    }
                    Err(e) => error!(error = %e, "context_assembler.observations_fallback_failed"),
                }
            }
        }

        debug!(
            souls = results.souls.len(),
            goals = results.goals.len(),
            memories = results.memories.len(),
            observations = results.observations.len(),
            "context_assembler.build_context"
        );

        results
    }

    // ── Individual retrieval methods for tools ──────────────────────────

    /// Search memories semantically.
    pub async fn get_memories(
        &self,
        query: &str,
        limit: i64,
        min_score: f64,
    ) -> Vec<Memory> {
        let emb = match self.embedding.embed(query).await {
            Ok(v) => v,
            Err(e) => {
                error!(error = %e, "context_assembler.get_memories_failed");
                return Vec::new();
            }
        };
        match self.memory_repo.find_similar(&emb, limit, true, min_score).await {
            Ok(pairs) => pairs.into_iter().map(|(m, _)| m).collect(),
            Err(e) => {
                error!(error = %e, "context_assembler.get_memories_failed");
                Vec::new()
            }
        }
    }

    /// Get all active goals ordered by priority.
    pub async fn get_active_goals(&self) -> Vec<Goal> {
        match self.goal_repo.find_active(true).await {
            Ok(goals) => goals,
            Err(e) => {
                error!(error = %e, "context_assembler.get_active_goals_failed");
                Vec::new()
            }
        }
    }

    /// Get all enabled soul documents.
    pub async fn get_enabled_souls(&self) -> Vec<Soul> {
        match self.soul_repo.find_enabled().await {
            Ok(souls) => souls,
            Err(e) => {
                error!(error = %e, "context_assembler.get_enabled_souls_failed");
                Vec::new()
            }
        }
    }

    /// Get recent observations, optionally filtered by semantic relevance.
    pub async fn get_recent_observations(
        &self,
        query: Option<&str>,
        limit: i64,
        minutes: i64,
    ) -> Vec<Observation> {
        match query {
            Some(q) if !q.is_empty() => {
                let emb = match self.embedding.embed(q).await {
                    Ok(v) => v,
                    Err(e) => {
                        error!(error = %e, "context_assembler.get_recent_observations_failed");
                        return Vec::new();
                    }
                };
                match self.observation_repo.find_similar(&emb, limit).await {
                    Ok(pairs) => pairs.into_iter().map(|(o, _)| o).collect(),
                    Err(e) => {
                        error!(error = %e, "context_assembler.get_recent_observations_failed");
                        Vec::new()
                    }
                }
            }
            _ => match self.observation_repo.find_recent(minutes).await {
                Ok(mut obs) => {
                    obs.truncate(limit as usize);
                    obs
                }
                Err(e) => {
                    error!(error = %e, "context_assembler.get_recent_observations_failed");
                    Vec::new()
                }
            },
        }
    }

    /// Get soul content using fallback chain: SoulService → repo → empty.
    pub async fn get_soul_content(&self) -> String {
        if let Some(ref svc) = self.soul_service {
            match svc.get_soul_async().await {
                Ok(content) => return content,
                Err(e) => warn!(error = %e, "context_assembler.soul_service_failed"),
            }
        }

        let souls = self.get_enabled_souls().await;
        if souls.is_empty() {
            return String::new();
        }
        souls
            .into_iter()
            .map(|s| s.content)
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

impl std::fmt::Debug for ContextAssembler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContextAssembler")
            .field("has_soul_service", &self.soul_service.is_some())
            .finish()
    }
}
