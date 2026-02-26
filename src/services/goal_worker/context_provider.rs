//! DefaultGoalContextProvider — assembles context relevant to a goal.
//!
//! Builds context from: semantic memory search, active goals, soul documents.

use std::sync::Arc;

use async_trait::async_trait;
use tracing::{debug, warn};

use crate::db::{GoalRepository, MemoryRepository, SoulRepository};
use crate::error::AppError;
use crate::llm::EmbeddingProvider;
use crate::models::goal::Goal;

use super::GoalContextProvider;

const MEMORY_SEARCH_LIMIT: i64 = 5;
const MEMORY_MIN_SCORE: f64 = 0.3;

/// Assembles context from memories, goals, and soul documents.
pub struct DefaultGoalContextProvider {
    memory_repo: Arc<dyn MemoryRepository>,
    goal_repo: Arc<dyn GoalRepository>,
    soul_repo: Arc<dyn SoulRepository>,
    embedding_provider: Arc<dyn EmbeddingProvider>,
}

impl DefaultGoalContextProvider {
    pub fn new(
        memory_repo: Arc<dyn MemoryRepository>,
        goal_repo: Arc<dyn GoalRepository>,
        soul_repo: Arc<dyn SoulRepository>,
        embedding_provider: Arc<dyn EmbeddingProvider>,
    ) -> Self {
        Self {
            memory_repo,
            goal_repo,
            soul_repo,
            embedding_provider,
        }
    }
}

#[async_trait]
impl GoalContextProvider for DefaultGoalContextProvider {
    async fn get_context_for_goal(&self, goal: &Goal) -> Result<String, AppError> {
        let mut sections: Vec<String> = Vec::new();

        // 1. Semantic memory search using goal content
        match self.embedding_provider.embed(&goal.content).await {
            Ok(embedding) => {
                match self
                    .memory_repo
                    .find_similar(&embedding, MEMORY_SEARCH_LIMIT, true, MEMORY_MIN_SCORE)
                    .await
                {
                    Ok(memories) if !memories.is_empty() => {
                        let mut mem_section = String::from("## Relevant Memories\n");
                        for (memory, score) in &memories {
                            mem_section
                                .push_str(&format!("- [score: {:.2}] {}\n", score, memory.content));
                        }
                        sections.push(mem_section);
                        debug!(
                            memory_count = memories.len(),
                            goal_id = %goal.id,
                            "goal_context.memories_found"
                        );
                    }
                    Ok(_) => {
                        debug!(goal_id = %goal.id, "goal_context.no_relevant_memories");
                    }
                    Err(e) => {
                        warn!(
                            error = %e,
                            goal_id = %goal.id,
                            "goal_context.memory_search_failed"
                        );
                    }
                }
            }
            Err(e) => {
                warn!(
                    error = %e,
                    goal_id = %goal.id,
                    "goal_context.embedding_failed"
                );
            }
        }

        // 2. Active goals (for awareness of other ongoing work)
        match self.goal_repo.find_active(true).await {
            Ok(active_goals) => {
                let other_goals: Vec<&Goal> =
                    active_goals.iter().filter(|g| g.id != goal.id).collect();
                if !other_goals.is_empty() {
                    let mut goals_section = String::from("## Other Active Goals\n");
                    for g in &other_goals {
                        goals_section.push_str(&format!("- [{}] {}\n", g.priority, g.content));
                    }
                    sections.push(goals_section);
                }
            }
            Err(e) => {
                warn!(error = %e, "goal_context.active_goals_failed");
            }
        }

        // 3. Soul documents (personality / instructions)
        match self.soul_repo.find_enabled().await {
            Ok(souls) if !souls.is_empty() => {
                let mut soul_section = String::from("## Soul / Identity\n");
                for soul in &souls {
                    soul_section.push_str(&format!("### {}\n{}\n\n", soul.name, soul.content));
                }
                sections.push(soul_section);
            }
            Ok(_) => {}
            Err(e) => {
                warn!(error = %e, "goal_context.soul_fetch_failed");
            }
        }

        if sections.is_empty() {
            Ok("No additional context available.".to_string())
        } else {
            Ok(sections.join("\n"))
        }
    }
}
