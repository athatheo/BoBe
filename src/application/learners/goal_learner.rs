//! Goal learner — detects and extracts goals from conversations.
//!
//! Deduplicates via semantic search + LLM decision (CREATE / UPDATE / SKIP).

use std::sync::Arc;

use serde_json::Value;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::application::learning::config::LearningConfig;
use crate::application::prompts::learning::deduplication_decision::GoalDeduplicationPrompt;
use crate::application::prompts::learning::goal_extraction::GoalExtractionPrompt;
use crate::application::services::goals::goals_service::GoalsService;
use crate::domain::goal::Goal;
use crate::domain::types::{GoalPriority, GoalSource};
use crate::error::AppError;
use crate::ports::embedding::EmbeddingProvider;
use crate::ports::llm::LlmProvider;

/// Valid values for goal priorities.
const VALID_PRIORITIES: &[&str] = &["high", "medium", "low"];

/// Threshold for initial semantic search.
const SIMILARITY_SEARCH_THRESHOLD: f64 = 0.5;

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

pub struct GoalLearner {
    llm: Arc<dyn LlmProvider>,
    embedding: Arc<dyn EmbeddingProvider>,
    goals: Arc<GoalsService>,
    config: LearningConfig,
}

impl GoalLearner {
    pub fn new(
        llm: Arc<dyn LlmProvider>,
        embedding: Arc<dyn EmbeddingProvider>,
        goals: Arc<GoalsService>,
        config: LearningConfig,
    ) -> Self {
        Self { llm, embedding, goals, config }
    }

    pub fn update_config(&mut self, config: LearningConfig) {
        self.config = config;
    }

    pub fn update_llm(&mut self, llm: Arc<dyn LlmProvider>) {
        self.llm = llm;
    }

    /// Extract goals from a closed conversation.
    pub async fn extract_from_conversation(
        &self,
        conversation_turns: &[(String, String)],
        existing_goals: &[Goal],
    ) -> Vec<Goal> {
        if conversation_turns.is_empty() {
            return Vec::new();
        }

        let turn_strings: Vec<String> = conversation_turns
            .iter()
            .map(|(role, content)| format!("{role}: {content}"))
            .collect();
        let goal_strings: Vec<String> = existing_goals.iter().map(|g| g.content.clone()).collect();

        let messages = GoalExtractionPrompt::messages(&turn_strings, &goal_strings);
        let config = GoalExtractionPrompt::config();

        let response = match tokio::time::timeout(
            std::time::Duration::from_secs(240),
            self.llm.complete(&messages, None, config.response_format.as_ref(), config.temperature, config.max_tokens),
        ).await {
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
            Ok(data) => data.get("goals").and_then(|g| g.as_array()).cloned().unwrap_or_default(),
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
        _existing_goals: &[Goal],
    ) -> Vec<Goal> {
        let mut created: Vec<Goal> = Vec::new();
        let max_goals = self.config.max_goals_per_cycle as usize;

        for raw in raw_goals {
            if created.len() >= max_goals {
                break;
            }

            let content = raw.get("content").and_then(|c| c.as_str()).unwrap_or("").trim();
            if content.is_empty() {
                continue;
            }

            let priority_str = raw.get("priority").and_then(|p| p.as_str()).unwrap_or("medium");
            let priority_str = if VALID_PRIORITIES.contains(&priority_str) { priority_str } else { "medium" };

            let inference_reason = raw
                .get("inference_reason")
                .and_then(|r| r.as_str())
                .unwrap_or("Detected from conversation")
                .to_owned();

            // Batch duplicate check
            let is_batch_dup = created.iter().any(|g| {
                g.content.to_lowercase().trim() == content.to_lowercase().trim()
            });
            if is_batch_dup {
                continue;
            }

            // LLM-based deduplication
            let decision = self.evaluate_goal(content).await;

            match decision {
                DeduplicationDecision::Skip => {
                    debug!(content_preview = &content[..content.len().min(50)], "goal_learner.skipped");
                    continue;
                }
                DeduplicationDecision::Update { existing_goal_id, updated_content } => {
                    if let Err(e) = self.update_existing_goal(existing_goal_id, &updated_content).await {
                        warn!(error = %e, "goal_learner.update_failed");
                    } else {
                        info!(goal_id = %existing_goal_id, "goal_learner.goal_updated");
                    }
                    continue;
                }
                DeduplicationDecision::Create => {
                    let priority = match priority_str {
                        "high" => GoalPriority::High,
                        "low" => GoalPriority::Low,
                        _ => GoalPriority::Medium,
                    };

                    match self.goals.create(content, GoalSource::Inferred, priority, Some(inference_reason)).await {
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

    async fn evaluate_goal(&self, content: &str) -> DeduplicationDecision {
        // Search for similar goals
        let query_embedding = match self.embedding.embed(content).await {
            Ok(e) => e,
            Err(e) => {
                warn!(error = %e, "goal_learner.embed_failed");
                return DeduplicationDecision::Create;
            }
        };

        let similar_goals = match self.goals.get_by_embedding(
            &query_embedding,
            5,
            SIMILARITY_SEARCH_THRESHOLD,
            None,
        ).await {
            Ok(g) => g,
            Err(_) => return DeduplicationDecision::Create,
        };

        if similar_goals.is_empty() {
            return DeduplicationDecision::Create;
        }

        let existing_data: Vec<(String, String, String)> = similar_goals
            .iter()
            .map(|g| (g.id.to_string(), g.content.clone(), g.priority.as_str().to_owned()))
            .collect();

        let messages = GoalDeduplicationPrompt::messages(content, &existing_data);
        let config = GoalDeduplicationPrompt::config();

        let response = match tokio::time::timeout(
            std::time::Duration::from_secs(240),
            self.llm.complete(&messages, None, config.response_format.as_ref(), config.temperature, config.max_tokens),
        ).await {
            Ok(Ok(r)) => r,
            _ => return DeduplicationDecision::Create,
        };

        let resp_content = response.message.content.text_or_empty().to_string();
        let data = match serde_json::from_str::<Value>(&resp_content) {
            Ok(d) => d,
            Err(_) => return DeduplicationDecision::Create,
        };

        let decision = data.get("decision").and_then(|d| d.as_str()).unwrap_or("CREATE").to_uppercase();

        match decision.as_str() {
            "SKIP" => DeduplicationDecision::Skip,
            "UPDATE" => {
                let raw_id = data.get("existing_goal_id").and_then(|v| v.as_str());
                let updated_content = data.get("updated_content").and_then(|v| v.as_str());

                match (raw_id, updated_content) {
                    (Some(id_str), Some(uc)) => {
                        match Uuid::parse_str(id_str) {
                            Ok(goal_id) => {
                                let valid_ids: Vec<String> = existing_data.iter().map(|(id, _, _)| id.clone()).collect();
                                if valid_ids.contains(&goal_id.to_string()) {
                                    let truncated = &uc[..uc.len().min(MAX_GOAL_CONTENT_LENGTH)];
                                    DeduplicationDecision::Update {
                                        existing_goal_id: goal_id,
                                        updated_content: truncated.trim().to_owned(),
                                    }
                                } else {
                                    DeduplicationDecision::Create
                                }
                            }
                            Err(_) => DeduplicationDecision::Create,
                        }
                    }
                    _ => DeduplicationDecision::Create,
                }
            }
            _ => DeduplicationDecision::Create,
        }
    }

    async fn update_existing_goal(&self, goal_id: Uuid, content: &str) -> Result<(), AppError> {
        self.goals.update_content(goal_id, content).await?;
        Ok(())
    }
}
