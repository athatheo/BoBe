//! Goal learner — detects and extracts goals from conversations.
//!
//! Deduplicates via semantic search + LLM decision (CREATE / UPDATE / SKIP).

use std::sync::Arc;

use arc_swap::ArcSwap;
use serde_json::Value;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::config::Config;
use crate::error::AppError;
use crate::llm::EmbeddingProvider;
use crate::llm::LlmProvider;
use crate::models::goal::Goal;
use crate::models::types::{GoalPriority, GoalSource};
use crate::runtime::prompts::learning::deduplication_decision::GoalDeduplicationPrompt;
use crate::runtime::prompts::learning::goal_extraction::GoalExtractionPrompt;
use crate::services::goals::goals_service::GoalsService;

/// Valid values for goal priorities.
const VALID_PRIORITIES: &[&str] = &["high", "medium", "low"];

/// Threshold for initial semantic search.
const SIMILARITY_SEARCH_THRESHOLD: f64 = 0.35;

/// Maximum length for updated goal content from LLM.
const MAX_GOAL_CONTENT_LENGTH: usize = 10_000;

#[derive(Debug)]
enum DeduplicationDecision {
    Create,
    Update {
        existing_goal_id: Uuid,
        updated_content: String,
    },
    Skip,
}

pub(crate) struct GoalLearner {
    llm: Arc<dyn LlmProvider>,
    embedding: Arc<dyn EmbeddingProvider>,
    goals: Arc<GoalsService>,
    config: Arc<ArcSwap<Config>>,
}

impl GoalLearner {
    pub(crate) fn new(
        llm: Arc<dyn LlmProvider>,
        embedding: Arc<dyn EmbeddingProvider>,
        goals: Arc<GoalsService>,
        config: Arc<ArcSwap<Config>>,
    ) -> Self {
        Self {
            llm,
            embedding,
            goals,
            config,
        }
    }

    /// Extract goals from a closed conversation.
    pub(crate) async fn extract_from_conversation(
        &self,
        conversation_turns: &[(String, String)],
        existing_goals: &[Goal],
    ) -> Vec<Goal> {
        if conversation_turns.is_empty() {
            return Vec::new();
        }

        // Short conversations produce spurious goals — skip extraction
        if conversation_turns.len() < 4 {
            debug!(
                turns = conversation_turns.len(),
                "goal_learner.skipping_short_conversation"
            );
            return Vec::new();
        }

        let turn_strings: Vec<String> = conversation_turns
            .iter()
            .map(|(role, content)| format!("{role}: {content}"))
            .collect();
        let goal_strings: Vec<String> = existing_goals.iter().map(|g| g.content.clone()).collect();
        let locale = self.config.load().effective_locale();

        let messages = GoalExtractionPrompt::messages(&turn_strings, &goal_strings, Some(&locale));
        let prompt_config = GoalExtractionPrompt::config();

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
                warn!(error = %e, "goal_learner.llm_error");
                return Vec::new();
            }
            Err(_) => {
                warn!("goal_learner.llm_timeout");
                return Vec::new();
            }
        };

        let content = response.message.content.text_or_empty().to_string();
        if content.trim().is_empty() {
            return Vec::new();
        }

        let raw_goals = match serde_json::from_str::<Value>(&content) {
            Ok(data) => data
                .get("goals")
                .and_then(|g| g.as_array())
                .cloned()
                .unwrap_or_default(),
            Err(e) => {
                warn!(error = %e, "goal_learner.json_parse_error");
                return Vec::new();
            }
        };

        self.deduplicate_and_store(&raw_goals, existing_goals).await
    }

    async fn deduplicate_and_store(
        &self,
        raw_goals: &[Value],
        existing_goals: &[Goal],
    ) -> Vec<Goal> {
        let cfg = self.config.load();
        let mut created: Vec<Goal> = Vec::new();
        let max_goals = cfg.learning.max_goals_per_cycle as usize;
        let max_dedup_calls = max_goals * 2;
        let mut dedup_calls = 0usize;

        for raw in raw_goals {
            if created.len() >= max_goals {
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

            let priority_str = raw
                .get("priority")
                .and_then(|p| p.as_str())
                .unwrap_or("medium");
            let priority_str = if VALID_PRIORITIES.contains(&priority_str) {
                priority_str
            } else {
                "medium"
            };

            let inference_reason = raw
                .get("inference_reason")
                .and_then(|r| r.as_str())
                .unwrap_or("Detected from conversation")
                .to_owned();

            // Batch duplicate check
            let is_batch_dup = created
                .iter()
                .any(|g| g.content.to_lowercase().trim() == content.to_lowercase().trim());
            if is_batch_dup {
                continue;
            }

            // Bound LLM dedup calls to prevent excessive API usage
            if dedup_calls >= max_dedup_calls {
                debug!(
                    max_dedup_calls = max_dedup_calls,
                    "goal_learner.max_dedup_calls_reached"
                );
                break;
            }

            // LLM-based deduplication
            dedup_calls += 1;
            let decision = self.evaluate_goal(content, existing_goals).await;

            match decision {
                DeduplicationDecision::Skip => {
                    debug!(
                        content_preview = &content[..content.len().min(50)],
                        "goal_learner.skipped"
                    );
                }
                DeduplicationDecision::Update {
                    existing_goal_id,
                    updated_content,
                } => {
                    if let Err(e) = self
                        .update_existing_goal(existing_goal_id, &updated_content)
                        .await
                    {
                        warn!(error = %e, "goal_learner.update_failed");
                    } else {
                        info!(goal_id = %existing_goal_id, "goal_learner.goal_updated");
                    }
                }
                DeduplicationDecision::Create => {
                    let priority = match priority_str {
                        "high" => GoalPriority::High,
                        "low" => GoalPriority::Low,
                        _ => GoalPriority::Medium,
                    };

                    match self
                        .goals
                        .create(
                            content,
                            GoalSource::Inferred,
                            priority,
                            Some(inference_reason),
                        )
                        .await
                    {
                        Ok(goal) => {
                            info!(goal_id = %goal.id, "goal_learner.goal_stored");
                            created.push(goal);
                        }
                        Err(e) => {
                            warn!(error = %e, "goal_learner.create_failed");
                        }
                    }
                }
            }
        }

        created
    }

    async fn evaluate_goal(&self, content: &str, existing_goals: &[Goal]) -> DeduplicationDecision {
        // Search for similar goals
        let query_embedding = match self.embedding.embed(content).await {
            Ok(e) => e,
            Err(e) => {
                warn!(error = %e, "goal_learner.embed_failed");
                return Self::fallback_decision(content, existing_goals, "embed_failed");
            }
        };

        let similar_goals = match self
            .goals
            .get_by_embedding(&query_embedding, 5, SIMILARITY_SEARCH_THRESHOLD, None)
            .await
        {
            Ok(goals) => goals,
            Err(e) => {
                warn!(error = %e, "goal_learner.similarity_search_failed");
                return Self::fallback_decision(
                    content,
                    existing_goals,
                    "similarity_search_failed",
                );
            }
        };

        if similar_goals.is_empty() {
            return DeduplicationDecision::Create;
        }

        let existing_data: Vec<(String, String, String)> = similar_goals
            .iter()
            .map(|g| {
                (
                    g.id.to_string(),
                    g.content.clone(),
                    g.priority.as_str().to_owned(),
                )
            })
            .collect();

        let locale = self.config.load().effective_locale();
        let messages = GoalDeduplicationPrompt::messages(content, &existing_data, Some(&locale));
        let prompt_config = GoalDeduplicationPrompt::config();

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
            Ok(Ok(response)) => response,
            Ok(Err(e)) => {
                warn!(error = %e, "goal_learner.dedup_llm_error");
                return Self::fallback_decision(content, &similar_goals, "dedup_llm_error");
            }
            Err(_) => {
                warn!("goal_learner.dedup_llm_timeout");
                return Self::fallback_decision(content, &similar_goals, "dedup_llm_timeout");
            }
        };

        let resp_content = response.message.content.text_or_empty().to_string();
        let data = match serde_json::from_str::<Value>(&resp_content) {
            Ok(data) => data,
            Err(e) => {
                warn!(error = %e, "goal_learner.dedup_json_parse_error");
                return Self::fallback_decision(content, &similar_goals, "dedup_json_parse_error");
            }
        };

        let decision = data
            .get("decision")
            .and_then(|d| d.as_str())
            .unwrap_or("")
            .to_uppercase();

        match decision.as_str() {
            "UPDATE" => {
                let raw_id = data.get("existing_goal_id").and_then(|v| v.as_str());
                let updated_content = data.get("updated_content").and_then(|v| v.as_str());

                match (raw_id, updated_content) {
                    (Some(id_str), Some(uc)) => match Uuid::parse_str(id_str) {
                        Ok(goal_id) => {
                            let valid_ids: Vec<String> =
                                existing_data.iter().map(|(id, _, _)| id.clone()).collect();
                            if valid_ids.contains(&goal_id.to_string()) {
                                let truncated = &uc[..uc.len().min(MAX_GOAL_CONTENT_LENGTH)];
                                DeduplicationDecision::Update {
                                    existing_goal_id: goal_id,
                                    updated_content: truncated.trim().to_owned(),
                                }
                            } else {
                                Self::fallback_decision(
                                    content,
                                    &similar_goals,
                                    "dedup_update_target_unknown",
                                )
                            }
                        }
                        Err(_) => Self::fallback_decision(
                            content,
                            &similar_goals,
                            "dedup_update_id_invalid",
                        ),
                    },
                    _ => Self::fallback_decision(content, &similar_goals, "dedup_update_invalid"),
                }
            }
            "CREATE" => DeduplicationDecision::Create,
            "SKIP" => DeduplicationDecision::Skip,
            _ => {
                warn!(decision = %decision, "goal_learner.dedup_unknown_decision");
                Self::fallback_decision(content, &similar_goals, "dedup_unknown_decision")
            }
        }
    }

    fn fallback_decision(
        content: &str,
        goals: &[Goal],
        reason: &'static str,
    ) -> DeduplicationDecision {
        if goals
            .iter()
            .any(|goal| Self::has_obvious_overlap(content, &goal.content))
        {
            warn!(
                reason,
                compared_goals = goals.len(),
                content_preview = %Self::preview(content),
                "goal_learner.dedup_fallback_skip"
            );
            DeduplicationDecision::Skip
        } else {
            warn!(
                reason,
                compared_goals = goals.len(),
                content_preview = %Self::preview(content),
                "goal_learner.dedup_fallback_create"
            );
            DeduplicationDecision::Create
        }
    }

    fn has_obvious_overlap(candidate: &str, existing: &str) -> bool {
        let candidate = Self::normalize_content(candidate);
        let existing = Self::normalize_content(existing);

        !candidate.is_empty()
            && !existing.is_empty()
            && (candidate == existing
                || candidate.contains(&existing)
                || existing.contains(&candidate))
    }

    fn normalize_content(content: &str) -> String {
        content
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_ascii_lowercase()
    }

    fn preview(content: &str) -> String {
        const MAX_CHARS: usize = 120;
        let mut out = String::new();
        for ch in content.chars().take(MAX_CHARS) {
            out.push(ch);
        }
        out
    }

    async fn update_existing_goal(&self, goal_id: Uuid, content: &str) -> Result<(), AppError> {
        self.goals.update_content(goal_id, content).await?;
        Ok(())
    }
}
