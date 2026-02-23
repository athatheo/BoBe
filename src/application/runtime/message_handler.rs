//! Message handler — handles incoming user messages.
//!
//! Creates/activates conversation, runs LLM with tools, streams response.

use std::sync::Arc;

use chrono::Utc;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::adapters::sse::event_queue::EventQueue;
use crate::adapters::sse::types::IndicatorType;
use crate::application::learners::types::LearnerObservation;
use crate::application::learners::MessageLearner;
use crate::application::prompts::response::UserResponsePrompt;
use crate::application::runtime::response_streamer::stream_llm_response;
use crate::application::runtime::state::OrchestratorConfig;
use crate::application::services::context_assembler::{BuildContextOptions, ContextAssembler};
use crate::application::services::conversation_service::ConversationService;
use crate::domain::types::TurnRole;
use crate::ports::llm::LlmProvider;
use crate::ports::repos::cooldown_repo::CooldownRepository;

pub struct MessageHandler {
    llm: Arc<dyn LlmProvider>,
    context_assembler: Arc<ContextAssembler>,
    conversation: Arc<ConversationService>,
    message_learner: Arc<MessageLearner>,
    cooldown_repo: Option<Arc<dyn CooldownRepository>>,
    event_queue: Arc<EventQueue>,
    config: OrchestratorConfig,
}

impl MessageHandler {
    pub fn new(
        llm: Arc<dyn LlmProvider>,
        context_assembler: Arc<ContextAssembler>,
        conversation: Arc<ConversationService>,
        message_learner: Arc<MessageLearner>,
        cooldown_repo: Option<Arc<dyn CooldownRepository>>,
        event_queue: Arc<EventQueue>,
        config: OrchestratorConfig,
    ) -> Self {
        Self {
            llm,
            context_assembler,
            conversation,
            message_learner,
            cooldown_repo,
            event_queue,
            config,
        }
    }

    pub fn update_config(&mut self, config: OrchestratorConfig) {
        self.config = config;
    }

    pub fn update_llm(&mut self, llm: Arc<dyn LlmProvider>) {
        self.llm = llm;
    }

    /// Handle a user message. Returns message ID for tracking.
    pub async fn handle_message(&self, content: &str) -> String {
        let msg_id = format!("msg_{}", Uuid::new_v4().simple());

        // 1. Record user activity for cooldown
        if let Some(ref cooldown_repo) = self.cooldown_repo {
            if let Err(e) = cooldown_repo.update_last_user_response(Utc::now()).await {
                warn!(error = %e, "message_handler.cooldown_update_failed");
            }
        }

        // 2. Conversation lifecycle
        let conversation_id = self.ensure_active_conversation(content).await;
        let Some(conversation_id) = conversation_id else {
            error!("message_handler.conversation_failed");
            return msg_id;
        };

        // 3. Learning - store message with embedding (fire-and-forget)
        let observation = LearnerObservation::message(content.to_owned());
        if let Err(e) = self.message_learner.learn(&observation).await {
            warn!(error = %e, "message_handler.learning_failed");
        }

        // 4. Generate response
        self.respond_to_message(&msg_id, content, conversation_id).await;

        msg_id
    }

    async fn ensure_active_conversation(&self, user_content: &str) -> Option<Uuid> {
        let existing = self.conversation.get_pending_or_active().await.ok()?;

        if let Some(conv) = existing {
            // Activate if pending
            if conv.is_pending() {
                self.conversation.activate(conv.id).await.ok();
            }

            // Add user turn
            match self.conversation.add_turn(conv.id, TurnRole::User, user_content).await {
                Ok(Some(_)) => Some(conv.id),
                _ => {
                    // Race condition: create new
                    match self.conversation.create_active(user_content).await {
                        Ok(new_conv) => Some(new_conv.id),
                        Err(e) => {
                            error!(error = %e, "message_handler.create_conversation_failed");
                            None
                        }
                    }
                }
            }
        } else {
            match self.conversation.create_active(user_content).await {
                Ok(new_conv) => Some(new_conv.id),
                Err(e) => {
                    error!(error = %e, "message_handler.create_conversation_failed");
                    None
                }
            }
        }
    }

    async fn respond_to_message(&self, msg_id: &str, user_content: &str, conversation_id: Uuid) {
        self.event_queue.set_indicator(IndicatorType::Streaming);

        // Get context
        let mut context_summary = String::new();
        let mut soul: Option<String> = None;

        let assembled = self.context_assembler.build_context(user_content, BuildContextOptions {
            include_memories: true,
            include_goals: true,
            include_souls: true,
            include_observations: true,
            memory_limit: 5,
            observation_limit: 5,
            ..BuildContextOptions::default()
        }).await;

        let (ctx, s) = assembled.to_context_string();
        context_summary = ctx;
        soul = s;

        // Get conversation history
        let mut conversation_history: Vec<(String, String)> = Vec::new();
        if let Ok(turns) = self.conversation.get_conversation_turns(conversation_id, 20).await {
            // If fresh conversation, load from previous
            if turns.len() <= 1 {
                let previous = self.conversation.get_previous_conversation_context().await;
                conversation_history.extend(previous);
            }

            // Add current turns (excluding last user message)
            let slice = if turns.is_empty() { &[] } else { &turns[..turns.len() - 1] };
            for turn in slice {
                conversation_history.push((turn.role.clone(), turn.content.clone()));
            }
        }

        let history_refs: Vec<(&str, &str)> = conversation_history
            .iter()
            .map(|(r, c)| (r.as_str(), c.as_str()))
            .collect();

        let messages = UserResponsePrompt::messages(
            user_content,
            &context_summary,
            if history_refs.is_empty() { None } else { Some(&history_refs) },
            soul.as_deref(),
        );
        let config = UserResponsePrompt::config();

        info!(
            context_len = context_summary.len(),
            history = conversation_history.len(),
            "message_handler.stream_start"
        );

        let stream = self.llm.stream(
            messages,
            None,
            config.response_format,
            config.temperature,
            config.max_tokens,
        );

        let result = stream_llm_response(stream, &self.event_queue, Some(msg_id)).await;

        // Persist response
        if result.success && !result.full_response.is_empty() {
            match self.conversation.get_conversation(conversation_id).await {
                Ok(Some(conv)) if !conv.is_closed() => {
                    if let Err(e) = self.conversation.add_turn(
                        conversation_id, TurnRole::Assistant, &result.full_response,
                    ).await {
                        error!(error = %e, "message_handler.persist_failed");
                    }
                }
                _ => {
                    error!(msg_id = %msg_id, "message_handler.conversation_closed_before_persist");
                }
            }
        }

        self.event_queue.set_indicator(IndicatorType::Idle);
    }
}
