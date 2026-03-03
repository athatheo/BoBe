//! Message handler — handles incoming user messages.
//!
//! Creates/activates conversation, runs LLM with tools, streams response.

use std::sync::Arc;

use arc_swap::ArcSwap;
use chrono::Utc;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::config::Config;
use crate::db::CooldownRepository;
use crate::llm::LlmProvider;
use crate::models::types::TurnRole;
use crate::runtime::learners::MessageLearner;
use crate::runtime::learners::types::LearnerObservation;
use crate::runtime::prompts::response::UserResponsePrompt;
use crate::runtime::response_streamer::{stream_llm_response, stream_response};
use crate::services::context_assembler::{BuildContextOptions, ContextAssembler};
use crate::services::conversation_service::ConversationService;
use crate::tools::ToolExecutionContext;
use crate::tools::preselector::ToolPreselector;
use crate::tools::registry::ToolRegistry;
use crate::tools::tool_call_loop::ToolCallLoop;
use crate::util::sse::event_queue::EventQueue;
use crate::util::sse::types::IndicatorType;
use crate::util::tokens::{clamp_max_tokens, count_message_tokens};

pub struct MessageHandler {
    llm: Arc<dyn LlmProvider>,
    context_assembler: Arc<ContextAssembler>,
    conversation: Arc<ConversationService>,
    message_learner: Arc<MessageLearner>,
    cooldown_repo: Option<Arc<dyn CooldownRepository>>,
    event_queue: Arc<EventQueue>,
    config: Arc<ArcSwap<Config>>,
    tool_registry: Option<Arc<ToolRegistry>>,
    tool_preselector: Option<Arc<ToolPreselector>>,
    tool_call_loop: Option<Arc<ToolCallLoop>>,
}

impl MessageHandler {
    pub fn new(
        llm: Arc<dyn LlmProvider>,
        context_assembler: Arc<ContextAssembler>,
        conversation: Arc<ConversationService>,
        message_learner: Arc<MessageLearner>,
        cooldown_repo: Option<Arc<dyn CooldownRepository>>,
        event_queue: Arc<EventQueue>,
        config: Arc<ArcSwap<Config>>,
        tool_registry: Option<Arc<ToolRegistry>>,
        tool_preselector: Option<Arc<ToolPreselector>>,
        tool_call_loop: Option<Arc<ToolCallLoop>>,
    ) -> Self {
        Self {
            llm,
            context_assembler,
            conversation,
            message_learner,
            cooldown_repo,
            event_queue,
            config,
            tool_registry,
            tool_preselector,
            tool_call_loop,
        }
    }

    /// Handle a user message. Returns message ID for tracking.
    pub async fn handle_message(&self, content: &str, message_id: &str) {
        // 1. Record user activity for cooldown
        if let Some(ref cooldown_repo) = self.cooldown_repo
            && let Err(e) = cooldown_repo.update_last_user_response(Utc::now()).await
        {
            warn!(error = %e, "message_handler.cooldown_update_failed");
        }

        // 2. Conversation lifecycle
        let conversation_id = self.ensure_active_conversation(content).await;
        let Some(conversation_id) = conversation_id else {
            error!("message_handler.conversation_failed");
            return;
        };

        // 3. Learning - store message with embedding (fire-and-forget)
        let observation = LearnerObservation::message(content.to_owned());
        if let Err(e) = self.message_learner.learn(&observation).await {
            warn!(error = %e, "message_handler.learning_failed");
        }

        // 4. Generate response
        self.respond_to_message(message_id, content, conversation_id)
            .await;
    }

    async fn ensure_active_conversation(&self, user_content: &str) -> Option<Uuid> {
        let existing = self.conversation.get_pending_or_active().await.ok()?;

        if let Some(conv) = existing {
            if conv.is_pending() {
                self.conversation.activate(conv.id).await.ok();
            }

            match self
                .conversation
                .add_turn(conv.id, TurnRole::User, user_content)
                .await
            {
                Ok(Some(_)) => Some(conv.id),
                _ => match self.conversation.create_active(user_content).await {
                    Ok(new_conv) => Some(new_conv.id),
                    Err(e) => {
                        error!(error = %e, "message_handler.create_conversation_failed");
                        None
                    }
                },
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
        let cfg = self.config.load();
        self.event_queue.set_indicator(IndicatorType::Streaming);

        // Build context and conversation history
        let assembled = self
            .context_assembler
            .build_context(
                user_content,
                BuildContextOptions {
                    include_memories: true,
                    include_goals: true,
                    include_souls: true,
                    include_observations: true,
                    memory_limit: 5,
                    observation_limit: 5,
                    ..BuildContextOptions::default()
                },
            )
            .await;

        let (context_summary, soul) = assembled.to_context_string();
        let conversation_history = self.build_conversation_history(conversation_id).await;

        let history_refs: Vec<(&str, &str)> = conversation_history
            .iter()
            .map(|(r, c)| (r.as_str(), c.as_str()))
            .collect();

        let messages = UserResponsePrompt::messages(
            user_content,
            &context_summary,
            if history_refs.is_empty() {
                None
            } else {
                Some(&history_refs)
            },
            soul.as_deref(),
        );
        let prompt_config = UserResponsePrompt::config();

        // Get tools if enabled
        let tools = if cfg.tools.enabled {
            self.get_tools(&messages).await
        } else {
            Vec::new()
        };

        // Clamp max_tokens so prompt + response fits within context window
        let prompt_tokens = count_message_tokens(&messages);
        let context_window = cfg.llm.context_window;
        let max_tokens = clamp_max_tokens(context_window, prompt_tokens, prompt_config.max_tokens);
        if max_tokens < prompt_config.max_tokens {
            info!(
                requested = prompt_config.max_tokens,
                clamped = max_tokens,
                prompt_tokens,
                context_window,
                "message_handler.max_tokens_clamped"
            );
        }

        info!(
            context_len = context_summary.len(),
            history = conversation_history.len(),
            tools = tools.len(),
            prompt_tokens,
            max_tokens,
            "message_handler.stream_start"
        );

        let tool_context = ToolExecutionContext {
            conversation_id: Some(conversation_id.to_string()),
        };

        // Stream with or without tools
        let result = if let (false, Some(tcl)) = (tools.is_empty(), self.tool_call_loop.as_ref()) {
            let stream = tcl.stream(
                messages,
                tools,
                prompt_config.temperature,
                max_tokens,
                Some(tool_context),
            );
            stream_response(stream, &self.event_queue, Some(msg_id)).await
        } else {
            let stream = self.llm.stream(
                messages,
                None,
                prompt_config.response_format,
                prompt_config.temperature,
                max_tokens,
            );
            stream_llm_response(stream, &self.event_queue, Some(msg_id)).await
        };

        self.persist_response(&result, conversation_id).await;
        self.event_queue.set_indicator(IndicatorType::Idle);
    }

    async fn build_conversation_history(&self, conversation_id: Uuid) -> Vec<(String, String)> {
        let mut history: Vec<(String, String)> = Vec::new();
        match self
            .conversation
            .get_conversation_turns(conversation_id, 20)
            .await
        {
            Ok(turns) => {
                if turns.len() <= 1 {
                    let previous = self.conversation.get_previous_conversation_context().await;
                    if !previous.is_empty() {
                        info!(
                            previous_turns = previous.len(),
                            "message_handler.loaded_previous_context"
                        );
                    }
                    history.extend(previous);
                }

                let slice = if turns.is_empty() {
                    &[]
                } else {
                    &turns[..turns.len() - 1]
                };
                for turn in slice {
                    history.push((turn.role.as_str().to_owned(), turn.content.clone()));
                }
            }
            Err(e) => {
                error!(
                    error = %e,
                    conversation_id = %conversation_id,
                    "message_handler.history_load_failed"
                );
            }
        }
        history
    }

    async fn persist_response(
        &self,
        result: &crate::runtime::response_streamer::StreamResult,
        conversation_id: Uuid,
    ) {
        if !result.success || result.full_response.is_empty() {
            return;
        }
        match self.conversation.get_conversation(conversation_id).await {
            Ok(Some(conv)) if !conv.is_closed() => {
                if let Err(e) = self
                    .conversation
                    .add_turn(conversation_id, TurnRole::Assistant, &result.full_response)
                    .await
                {
                    error!(error = %e, "message_handler.persist_failed");
                } else {
                    let chunks_per_sec = if result.duration_ms > 0.0 {
                        result.chunk_count as f64 / (result.duration_ms / 1000.0)
                    } else {
                        0.0
                    };
                    info!(
                        chunks = result.chunk_count,
                        ms = result.duration_ms as u64,
                        cps = format!("{chunks_per_sec:.1}"),
                        "message_handler.response_complete"
                    );
                }
            }
            Ok(Some(_)) => {
                warn!(
                    conversation_id = %conversation_id,
                    "message_handler.conversation_closed_before_persist"
                );
            }
            Ok(None) => {
                error!(conversation_id = %conversation_id, "message_handler.conversation_not_found");
            }
            Err(e) => {
                error!(error = %e, "message_handler.conversation_refetch_failed");
            }
        }
    }

    /// Get available tools, optionally preselected based on conversation context.
    async fn get_tools(
        &self,
        messages: &[crate::llm::types::AiMessage],
    ) -> Vec<crate::llm::types::ToolDefinition> {
        let Some(ref registry) = self.tool_registry else {
            return Vec::new();
        };

        let all_tools = registry.get_all_tools(true).await;
        if all_tools.is_empty() {
            return Vec::new();
        }

        let selected = if let Some(ref preselector) = self.tool_preselector {
            preselector.preselect(messages, &all_tools).await
        } else {
            all_tools.clone()
        };

        if !selected.is_empty() {
            info!(
                total = all_tools.len(),
                selected = selected.len(),
                "message_handler.tools_loaded"
            );
        }
        selected
    }
}
