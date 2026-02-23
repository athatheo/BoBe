//! Decision engine — decides whether to reach out proactively.
//!
//! Uses context, conversation state, and LLM structured output.

use std::sync::Arc;

use chrono::{Duration, Utc};
use serde_json::Value;
use tracing::{debug, warn};

use crate::application::prompts::decision::DecisionPrompt;
use crate::application::prompts::goal_decision::GoalDecisionPrompt;
use crate::application::runtime::state::{Decision, OrchestratorConfig, TriggerContext, TriggerType};
use crate::application::services::context_assembler::ContextAssembler;
use crate::application::services::conversation_service::ConversationService;
use crate::ports::llm::LlmProvider;
use crate::ports::repos::observation_repo::ObservationRepository;

pub struct DecisionEngine {
    llm: Arc<dyn LlmProvider>,
    observation_repo: Arc<dyn ObservationRepository>,
    conversation: Arc<ConversationService>,
    config: OrchestratorConfig,
    context_assembler: Option<Arc<ContextAssembler>>,
}

impl DecisionEngine {
    pub fn new(
        llm: Arc<dyn LlmProvider>,
        observation_repo: Arc<dyn ObservationRepository>,
        conversation: Arc<ConversationService>,
        config: OrchestratorConfig,
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

    pub fn update_config(&mut self, config: OrchestratorConfig) {
        self.config = config;
    }

    pub fn update_llm(&mut self, llm: Arc<dyn LlmProvider>) {
        self.llm = llm;
    }

    /// Route to appropriate decision logic based on trigger type.
    pub async fn decide(&self, context: &TriggerContext) -> Decision {
        match context.trigger_type {
            TriggerType::Capture => {
                let embedding = context.observation.as_ref()
                    .and_then(|obs| obs.embedding.as_ref())
                    .and_then(|e| serde_json::from_str::<Vec<f32>>(e).ok());
                self.decide_on_capture(&context.context_text, embedding.as_deref()).await
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
        // Check active conversation
        if let Ok(Some(active)) = self.conversation.get_pending_or_active().await {
            let timeout = Duration::seconds(self.config.conversation_inactivity_timeout_seconds as i64);
            let time_since = Utc::now() - active.updated_at;
            if time_since < timeout {
                debug!(
                    conversation_id = %active.id,
                    "decision_engine.blocked_by_recent_conversation"
                );
                return Decision::Idle;
            }
        }

        // Get recent AI messages
        let recent_ai_messages = self.conversation
            .get_recent_ai_messages(self.config.recent_ai_messages_limit)
            .await
            .unwrap_or_default();

        // Get similar observations via semantic search (fallback to recent)
        let similar_observations = match embedding {
            Some(emb) => {
                match self.observation_repo
                    .find_similar(emb, self.config.semantic_search_limit)
                    .await
                {
                    Ok(results) => {
                        debug!(
                            result_count = results.len(),
                            "decision_engine.semantic_search_complete"
                        );
                        results.into_iter().map(|(obs, _score)| obs).collect()
                    }
                    Err(e) => {
                        warn!(error = %e, "decision_engine.semantic_search_failed");
                        self.observation_repo.find_recent(10).await.unwrap_or_default()
                    }
                }
            }
            None => {
                debug!("decision_engine.no_embedding_fallback");
                self.observation_repo.find_recent(10).await.unwrap_or_default()
            }
        };

        // Check LLM health
        if !self.llm.health_check().await {
            warn!("decision_engine.llm_unhealthy");
            return Decision::Idle;
        }

        // Build context
        let context_lines: Vec<String> = similar_observations
            .iter()
            .take(5)
            .map(|obs| format!("- [{}] {}", obs.category, &obs.content[..obs.content.len().min(100)]))
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
                    let truncated = if msg.len() > 100 { format!("{}...", &msg[..100]) } else { msg.clone() };
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
        // Check active conversation
        if let Ok(Some(active)) = self.conversation.get_pending_or_active().await {
            let timeout = Duration::seconds(self.config.conversation_inactivity_timeout_seconds as i64);
            let time_since = Utc::now() - active.updated_at;
            if time_since < timeout {
                debug!("decision_engine.goal_blocked_by_conversation");
                return Decision::Idle;
            }
        }

        // Get recent observations for context
        let recent_observations = self.observation_repo
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
            .map(|obs| format!("- [{}] {}", obs.category, &obs.content[..obs.content.len().min(100)]))
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
            if content.is_empty() { None } else { Some(content) }
        } else {
            None
        }
    }

    async fn llm_decide(
        &self,
        messages: &[crate::ports::llm_types::AiMessage],
        config: &crate::application::prompts::base::PromptConfig,
    ) -> Decision {
        let response = match tokio::time::timeout(
            std::time::Duration::from_secs(240),
            self.llm.complete(messages, None, config.response_format.as_ref(), config.temperature, config.max_tokens),
        ).await {
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
        let content = content.trim();

        // Try JSON
        if let Ok(data) = serde_json::from_str::<Value>(content) {
            let decision_value = data.get("decision").and_then(|d| d.as_str()).unwrap_or("idle");
            let reasoning = data.get("reasoning").and_then(|r| r.as_str()).unwrap_or("");

            debug!(
                decision = %decision_value,
                reasoning = &reasoning[..reasoning.len().min(100)],
                "decision_engine.parsed_json"
            );

            return match decision_value {
                "reach_out" => Decision::Engage,
                "need_more_info" => Decision::NeedMoreInfo,
                _ => Decision::Idle,
            };
        }

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

    /// User messages always get a response.
    pub async fn decide_for_message(&self, _user_message: &str) -> Decision {
        Decision::Engage
    }
}
