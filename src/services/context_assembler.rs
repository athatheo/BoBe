//! ContextAssembler — single source of truth for LLM context retrieval.
//!
//! Coordinates retrieval from multiple sources (memories, goals, souls, observations)
//! to build context for LLM prompts. Read-only: create/update/delete goes through
//! specific services or repositories.

use std::sync::Arc;

use tracing::{debug, error, warn};

use crate::db::GoalRepository;
use crate::db::MemoryRepository;
use crate::db::ObservationRepository;
use crate::db::SoulRepository;
use crate::db::UserProfileRepository;
use crate::llm::EmbeddingProvider;
use crate::models::goal::Goal;
use crate::models::memory::Memory;
use crate::models::observation::Observation;
use crate::models::soul::Soul;
use crate::models::user_profile::UserProfile;
use crate::util::text::truncate_str;
use crate::util::tokens::count_tokens;

use super::soul_service::SoulService;

/// Token count above which we emit a warning log. Conservative baseline;
/// individual models may support much larger contexts (32K–128K+).
const CONTEXT_TOKEN_WARN_THRESHOLD: usize = 4_000;

/// Container for assembled context items from all sources.
#[derive(Debug, Clone, Default)]
pub struct AssembledContext {
    pub souls: Vec<Soul>,
    pub goals: Vec<Goal>,
    pub memories: Vec<Memory>,
    pub observations: Vec<Observation>,
    pub user_profiles: Vec<UserProfile>,
}

impl AssembledContext {
    /// Format assembled context for LLM prompt injection.
    /// Only includes sections that have content.
    pub fn to_prompt_sections(&self) -> std::collections::HashMap<String, String> {
        let mut sections = std::collections::HashMap::new();

        if !self.souls.is_empty() {
            let mut buf = String::new();
            for (i, s) in self.souls.iter().enumerate() {
                if i > 0 {
                    buf.push_str("\n\n");
                }
                buf.push_str(&s.content);
            }
            sections.insert("personality".into(), buf);
        }

        if !self.user_profiles.is_empty() {
            let mut buf = String::new();
            for (i, p) in self.user_profiles.iter().enumerate() {
                if i > 0 {
                    buf.push_str("\n\n");
                }
                buf.push_str(&p.content);
            }
            sections.insert("user_profile".into(), buf);
        }

        if !self.goals.is_empty() {
            use std::fmt::Write;
            let mut buf = String::new();
            for (i, g) in self.goals.iter().enumerate() {
                if i > 0 {
                    buf.push('\n');
                }
                let _ = write!(buf, "- {} (priority: {})", g.content, g.priority);
            }
            sections.insert("current_goals".into(), buf);
        }

        if !self.memories.is_empty() {
            use std::fmt::Write;
            let mut buf = String::new();
            for (i, m) in self.memories.iter().enumerate() {
                if i > 0 {
                    buf.push('\n');
                }
                let _ = write!(buf, "- {}", m.content);
            }
            sections.insert("relevant_memories".into(), buf);
        }

        if !self.observations.is_empty() {
            use std::fmt::Write;
            let mut buf = String::new();
            for (i, o) in self.observations.iter().enumerate() {
                if i > 0 {
                    buf.push('\n');
                }
                let category = if o.category.is_empty() {
                    "general"
                } else {
                    &o.category
                };
                let summary = o
                    .metadata
                    .as_ref()
                    .and_then(|m| serde_json::from_str::<serde_json::Value>(m).ok())
                    .and_then(|v| {
                        v.get("summary")
                            .and_then(|s| s.as_str())
                            .map(str::to_string)
                    });
                if let Some(ref s) = summary {
                    let _ = write!(buf, "- [{category}] {s}");
                } else if o.content.len() > 100 {
                    let _ = write!(buf, "- [{}] {}...", category, truncate_str(&o.content, 100));
                } else {
                    let _ = write!(buf, "- [{}] {}", category, o.content);
                }
            }
            sections.insert("recent_context".into(), buf);
        }

        sections
    }

    /// Format assembled context into a single context string and optional personality.
    pub fn to_context_string(&self) -> (String, Option<String>) {
        let mut sections = self.to_prompt_sections();
        let personality = sections.remove("personality");
        let mut context = sections.remove("recent_context").unwrap_or_default();

        if let Some(goals) = sections.get("current_goals") {
            context = format!("User goals:\n{goals}\n\n{context}");
        }
        if let Some(profile) = sections.get("user_profile") {
            context = format!("User profile:\n{profile}\n\n{context}");
        }
        if let Some(memories) = sections.get("relevant_memories") {
            context = format!("Relevant memories:\n{memories}\n\n{context}");
        }

        let token_count = count_tokens(&context);
        if token_count > CONTEXT_TOKEN_WARN_THRESHOLD {
            tracing::warn!(
                tokens = token_count,
                "context_assembler.context_large — may approach model limits"
            );
        }

        (context, personality)
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
    user_profile_repo: Arc<dyn UserProfileRepository>,
    embedding: Arc<dyn EmbeddingProvider>,
    soul_service: Option<Arc<SoulService>>,
}

impl ContextAssembler {
    pub fn new(
        soul_repo: Arc<dyn SoulRepository>,
        goal_repo: Arc<dyn GoalRepository>,
        memory_repo: Arc<dyn MemoryRepository>,
        observation_repo: Arc<dyn ObservationRepository>,
        user_profile_repo: Arc<dyn UserProfileRepository>,
        embedding: Arc<dyn EmbeddingProvider>,
        soul_service: Option<Arc<SoulService>>,
    ) -> Self {
        Self {
            soul_repo,
            goal_repo,
            memory_repo,
            observation_repo,
            user_profile_repo,
            embedding,
            soul_service,
        }
    }

    /// Build complete context for an LLM prompt.
    pub async fn build_context(&self, query: &str, opts: BuildContextOptions) -> AssembledContext {
        let mut results = AssembledContext::default();

        // Generate embedding once, reuse for all semantic searches
        let embedding = if !query.is_empty() && (opts.include_memories || opts.include_observations)
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
            match self.user_profile_repo.find_enabled().await {
                Ok(profiles) => results.user_profiles = profiles,
                Err(e) => error!(error = %e, "context_assembler.user_profiles_failed"),
            }
        }

        if opts.include_goals {
            match self.goal_repo.find_active(true).await {
                Ok(goals) => results.goals = goals,
                Err(e) => error!(error = %e, "context_assembler.goals_failed"),
            }
        }

        if opts.include_memories
            && let Some(ref emb) = embedding
        {
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
                match self
                    .observation_repo
                    .find_similar(emb, opts.observation_limit)
                    .await
                {
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
