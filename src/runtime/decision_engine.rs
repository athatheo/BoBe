//! Decision engine — decides whether to reach out proactively.
//!
//! Uses context, conversation state, and LLM structured output.

use std::sync::Arc;

use arc_swap::ArcSwap;
use chrono::{Duration, Utc};
use serde_json::Value;
use tracing::{debug, info, warn};

use crate::config::Config;
use crate::db::ObservationRepository;
use crate::llm::LlmProvider;
use crate::runtime::prompts::decision::DecisionPrompt;
use crate::runtime::prompts::goal_decision::GoalDecisionPrompt;
use crate::runtime::state::{Decision, TriggerContext, TriggerType};
use crate::services::context_assembler::ContextAssembler;
use crate::services::conversation_service::ConversationService;

pub struct DecisionEngine {
    llm: Arc<dyn LlmProvider>,
    observation_repo: Arc<dyn ObservationRepository>,
    conversation: Arc<ConversationService>,
    config: Arc<ArcSwap<Config>>,
    context_assembler: Option<Arc<ContextAssembler>>,
}

impl DecisionEngine {
    pub fn new(
        llm: Arc<dyn LlmProvider>,
        observation_repo: Arc<dyn ObservationRepository>,
        conversation: Arc<ConversationService>,
        config: Arc<ArcSwap<Config>>,
        context_assembler: Option<Arc<ContextAssembler>>,
    ) -> Self {
        Self {
            llm,
            observation_repo,
            conversation,
            config,
            context_assembler,
        }
    }

    /// Route to appropriate decision logic based on trigger type.
    pub async fn decide(&self, context: &TriggerContext) -> Decision {
        match context.trigger_type {
            TriggerType::Capture => {
                let embedding = context
                    .observation
                    .as_ref()
                    .and_then(|obs| obs.embedding.as_ref())
                    .and_then(|e| serde_json::from_str::<Vec<f32>>(e).ok());
                self.decide_on_capture(&context.context_text, embedding.as_deref())
                    .await
            }
            TriggerType::Goal => self.decide_on_goal(&context.context_text).await,
            TriggerType::Checkin => Decision::Engage,
            _ => {
                warn!(trigger_type = ?context.trigger_type, "decision_engine.unknown_trigger");
                Decision::Idle
            }
        }
    }

    async fn decide_on_capture(&self, current_text: &str, embedding: Option<&[f32]>) -> Decision {
        let cfg = self.config.load();

        // Check active conversation
        if let Ok(Some(active)) = self.conversation.get_pending_or_active().await {
            let timeout = Duration::seconds(cfg.conversation_inactivity_timeout_seconds as i64);
            let time_since = Utc::now() - active.updated_at;
            if time_since < timeout {
                debug!(
                    conversation_id = %active.id,
                    "decision_engine.blocked_by_recent_conversation"
                );
                return Decision::Idle;
            }
            // Conversation is stale but we proceed
            debug!(
                conversation_id = %active.id,
                stale_seconds = time_since.num_seconds(),
                "decision_engine.conversation_stale_allowing_reachout"
            );
        }

        // Get recent AI messages
        let recent_ai_messages = self
            .conversation
            .get_recent_ai_messages(cfg.recent_ai_messages_limit)
            .await
            .unwrap_or_default();

        // Get similar observations via semantic search with cascading fallback
        let similar_observations = self.get_similar_observations(embedding).await;

        // Check LLM health
        match tokio::time::timeout(std::time::Duration::from_secs(10), self.llm.health_check())
            .await
        {
            Ok(true) => {}
            Ok(false) => {
                warn!("decision_engine.llm_unhealthy");
                return Decision::Idle;
            }
            Err(_) => {
                warn!("decision_engine.llm_health_timeout");
                return Decision::Idle;
            }
        }

        // Build context using observation summaries
        let context_lines: Vec<String> = similar_observations
            .iter()
            .take(5)
            .map(|obs| {
                let summary = self.get_observation_summary(obs);
                format!("- [{}] {}", obs.category, summary)
            })
            .collect();
        let context_summary = if context_lines.is_empty() {
            "No recent context".into()
        } else {
            context_lines.join("\n")
        };

        let recent_messages = if recent_ai_messages.is_empty() {
            "I haven't sent any messages recently.".into()
        } else {
            let msgs: Vec<String> = recent_ai_messages
                .iter()
                .map(|msg| {
                    let truncated = if msg.len() > 100 {
                        format!("{}...", &msg[..100])
                    } else {
                        msg.clone()
                    };
                    format!("- {truncated}")
                })
                .collect();
            format!("Recent messages I sent:\n{}", msgs.join("\n"))
        };

        let current_summary = if current_text.len() < 200 {
            current_text.to_owned()
        } else {
            current_text[..200].to_owned()
        };

        // Get soul
        let soul = self.get_soul_content().await;

        let current_time = Utc::now().format("%A, %B %d %Y %H:%M").to_string();

        let messages = DecisionPrompt::messages(
            &current_summary,
            &context_summary,
            &recent_messages,
            soul.as_deref(),
            Some(&current_time),
        );
        let config = DecisionPrompt::config();

        self.llm_decide(&messages, &config).await
    }

    async fn decide_on_goal(&self, goal_content: &str) -> Decision {
        let cfg = self.config.load();

        // Check active conversation
        if let Ok(Some(active)) = self.conversation.get_pending_or_active().await {
            let timeout = Duration::seconds(cfg.conversation_inactivity_timeout_seconds as i64);
            let time_since = Utc::now() - active.updated_at;
            if time_since < timeout {
                debug!("decision_engine.goal_blocked_by_conversation");
                return Decision::Idle;
            }
        }

        // Get recent observations for context
        let recent_observations = self
            .observation_repo
            .find_recent(30)
            .await
            .unwrap_or_default();

        if !self.llm.health_check().await {
            warn!("decision_engine.goal_llm_unhealthy");
            return Decision::Idle;
        }

        let context_lines: Vec<String> = recent_observations
            .iter()
            .take(5)
            .map(|obs| {
                let summary = self.get_observation_summary(obs);
                format!("- [{}] {}", obs.category, summary)
            })
            .collect();
        let context_summary = if context_lines.is_empty() {
            "No recent context".into()
        } else {
            context_lines.join("\n")
        };

        let soul = self.get_soul_content().await;
        let current_time = Utc::now().format("%A, %B %d %Y %H:%M").to_string();

        let messages = GoalDecisionPrompt::messages(
            goal_content,
            &context_summary,
            soul.as_deref(),
            Some(&current_time),
        );
        let config = GoalDecisionPrompt::config();

        self.llm_decide(&messages, &config).await
    }

    async fn get_soul_content(&self) -> Option<String> {
        if let Some(ref ctx_asm) = self.context_assembler {
            let content = ctx_asm.get_soul_content().await;
            if content.is_empty() {
                None
            } else {
                Some(content)
            }
        } else {
            None
        }
    }

    async fn llm_decide(
        &self,
        messages: &[crate::llm::types::AiMessage],
        config: &crate::runtime::prompts::base::PromptConfig,
    ) -> Decision {
        let response = match tokio::time::timeout(
            std::time::Duration::from_secs(240),
            self.llm.complete(
                messages,
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
                warn!(error = %e, "decision_engine.llm_error");
                return Decision::Idle;
            }
            Err(_) => {
                warn!("decision_engine.llm_timeout");
                return Decision::Idle;
            }
        };

        self.parse_decision_response(response.message.content.text_or_empty())
    }

    fn parse_decision_response(&self, content: &str) -> Decision {
        if content.is_empty() {
            debug!("decision_engine.empty_content_idle");
            return Decision::Idle;
        }
        let content = content.trim();

        // Try JSON
        if let Ok(data) = serde_json::from_str::<Value>(content) {
            let decision_value = data
                .get("decision")
                .and_then(|d| d.as_str())
                .unwrap_or("idle");
            let reasoning = data.get("reasoning").and_then(|r| r.as_str()).unwrap_or("");

            debug!(
                decision = %decision_value,
                reasoning = &reasoning[..reasoning.len().min(150)],
                "decision_engine.parsed_json"
            );

            return match decision_value {
                "reach_out" => Decision::Engage,
                "need_more_info" => Decision::NeedMoreInfo,
                _ => Decision::Idle,
            };
        }

        // JSON parse failed
        warn!("decision_engine.json_parse_failed");

        // Fallback: text parsing
        let lower = content.to_lowercase();
        if lower.contains("reach_out") {
            Decision::Engage
        } else if lower.contains("need_more_info") || lower.contains("need more") {
            Decision::NeedMoreInfo
        } else {
            Decision::Idle
        }
    }

    /// Get similar observations via semantic search with cascading fallback.
    /// Fallback 1: recent observations (10 minutes). Fallback 2: empty.
    async fn get_similar_observations(
        &self,
        embedding: Option<&[f32]>,
    ) -> Vec<crate::models::observation::Observation> {
        let cfg = self.config.load();
        match embedding {
            Some(emb) => {
                let start = std::time::Instant::now();
                match self
                    .observation_repo
                    .find_similar(emb, cfg.semantic_search_limit)
                    .await
                {
                    Ok(results) => {
                        let duration_ms = start.elapsed().as_secs_f64() * 1000.0;
                        if !results.is_empty() {
                            let top_scores: Vec<f64> = results
                                .iter()
                                .take(3)
                                .map(|(_, s)| (*s * 1000.0).round() / 1000.0)
                                .collect();
                            info!(
                                result_count = results.len(),
                                top_scores = ?top_scores,
                                duration_ms = format!("{duration_ms:.1}"),
                                "decision_engine.semantic_search_complete"
                            );
                        } else {
                            debug!(
                                duration_ms = format!("{duration_ms:.1}"),
                                "decision_engine.semantic_search_empty"
                            );
                        }
                        results.into_iter().map(|(obs, _score)| obs).collect()
                    }
                    Err(e) => {
                        warn!(error = %e, "decision_engine.semantic_search_failed");
                        // Fallback 1: recent observations
                        match self.observation_repo.find_recent(10).await {
                            Ok(obs) => {
                                debug!(
                                    count = obs.len(),
                                    "decision_engine.fallback_recent_observations"
                                );
                                obs
                            }
                            Err(_) => Vec::new(),
                        }
                    }
                }
            }
            None => {
                debug!("decision_engine.no_embedding_fallback");
                match self.observation_repo.find_recent(10).await {
                    Ok(obs) => obs,
                    Err(e) => {
                        warn!(error = %e, "decision_engine.get_recent_failed");
                        Vec::new()
                    }
                }
            }
        }
    }

    /// Extract summary from observation metadata, or truncate content.
    fn get_observation_summary(&self, obs: &crate::models::observation::Observation) -> String {
        // Check metadata for pre-computed summary
        if let Some(ref meta) = obs.metadata
            && let Ok(parsed) = serde_json::from_str::<Value>(meta)
            && let Some(summary) = parsed.get("summary").and_then(|s| s.as_str())
        {
            let summary = summary.to_string();
            if summary.len() < 200 {
                return summary;
            }
            return summary[..200].to_string();
        }
        // Fallback: truncate content
        if obs.content.len() > 100 {
            obs.content[..100].to_string()
        } else {
            obs.content.clone()
        }
    }
}
