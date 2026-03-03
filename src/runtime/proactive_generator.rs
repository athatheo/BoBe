//! Proactive generator — generates proactive responses and check-in messages.
//!
//! Owns the complete proactive response lifecycle:
//! conversation setup, LLM generation, engagement recording.

use std::sync::Arc;

use arc_swap::ArcSwap;
use chrono::Utc;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::config::Config;
use crate::db::CooldownRepository;
use crate::llm::LlmProvider;
use crate::models::conversation::Conversation;
use crate::models::types::TurnRole;
use crate::runtime::prompts::response::ProactiveResponsePrompt;
use crate::runtime::prompts::summary::ConversationSummaryPrompt;
use crate::runtime::response_streamer::{stream_llm_response, stream_response};
use crate::services::context_assembler::{BuildContextOptions, ContextAssembler};
use crate::services::conversation_service::ConversationService;
use crate::tools::registry::ToolRegistry;
use crate::tools::tool_call_loop::ToolCallLoop;
use crate::util::sse::event_queue::EventQueue;
use crate::util::sse::factories::conversation_closed_event;
use crate::util::sse::types::IndicatorType;

pub struct ProactiveGenerator {
    llm: Arc<dyn LlmProvider>,
    context_assembler: Arc<ContextAssembler>,
    conversation: Arc<ConversationService>,
    event_queue: Arc<EventQueue>,
    config: Arc<ArcSwap<Config>>,
    cooldown_repo: Option<Arc<dyn CooldownRepository>>,
    tool_registry: Option<Arc<ToolRegistry>>,
    tool_call_loop: Option<Arc<ToolCallLoop>>,
}

impl ProactiveGenerator {
    pub fn new(
        llm: Arc<dyn LlmProvider>,
        context_assembler: Arc<ContextAssembler>,
        conversation: Arc<ConversationService>,
        event_queue: Arc<EventQueue>,
        config: Arc<ArcSwap<Config>>,
        cooldown_repo: Option<Arc<dyn CooldownRepository>>,
        tool_registry: Option<Arc<ToolRegistry>>,
        tool_call_loop: Option<Arc<ToolCallLoop>>,
    ) -> Self {
        Self {
            llm,
            context_assembler,
            conversation,
            event_queue,
            config,
            cooldown_repo,
            tool_registry,
            tool_call_loop,
        }
    }

    /// Complete proactive flow: conversation setup → generate → record engagement.
    pub async fn generate_proactive_response(
        &self,
        auto_close_minutes: i64,
        context_summary: Option<String>,
    ) {
        let (target, previous_summary) = self.ensure_conversation(auto_close_minutes).await;
        self.generate_response(target, previous_summary, context_summary)
            .await;
    }

    async fn ensure_conversation(
        &self,
        auto_close_minutes: i64,
    ) -> (Option<Conversation>, Option<String>) {
        let existing = self
            .conversation
            .get_pending_or_active()
            .await
            .ok()
            .flatten();

        if let Some(ref conv) = existing {
            if let Ok(turns) = self.conversation.get_conversation_turns(conv.id, 100).await
                && conv.is_stale(auto_close_minutes, &turns)
            {
                let old_id = conv.id.to_string();
                let old_turn_count = turns.len() as u32;
                let (new_conv, summary) = self.transition_conversation(conv).await;
                // Notify clients that the old conversation was closed
                self.event_queue.push(conversation_closed_event(
                    &old_id,
                    "inactivity_timeout",
                    old_turn_count,
                ));
                return (Some(new_conv), summary);
            }
            return (existing, None);
        }

        let last_closed = self
            .conversation
            .get_last_closed_conversation()
            .await
            .ok()
            .flatten();
        let previous_summary = last_closed.and_then(|c| c.summary);
        (None, previous_summary)
    }

    async fn generate_response(
        &self,
        target_conversation: Option<Conversation>,
        previous_summary: Option<String>,
        context_summary: Option<String>,
    ) {
        self.event_queue.set_indicator(IndicatorType::Streaming);

        // Build context
        let query = context_summary.as_deref().unwrap_or("");

        let assembled = self
            .context_assembler
            .build_context(
                query,
                BuildContextOptions {
                    include_memories: true,
                    include_goals: true,
                    include_souls: true,
                    include_observations: true,
                    observation_limit: 5,
                    ..BuildContextOptions::default()
                },
            )
            .await;

        let (assembled_context, soul) = assembled.to_context_string();

        let final_context = match &context_summary {
            Some(cs) => format!("{cs}\n\n{assembled_context}"),
            None => assembled_context,
        };

        let msg_id = format!("msg_{}", Uuid::new_v4().simple());
        let current_time = Utc::now().format("%A, %B %d %Y %H:%M").to_string();

        let messages = ProactiveResponsePrompt::messages(
            &final_context,
            soul.as_deref(),
            previous_summary.as_deref(),
            Some(&current_time),
        );
        let prompt_config = ProactiveResponsePrompt::config();

        // Load tools if registry available
        let tools = if let Some(ref registry) = self.tool_registry {
            registry.get_all_tools(false).await
        } else {
            vec![]
        };

        // Use tool call loop if tools are available, otherwise plain LLM stream
        let result = if let (false, Some(tcl)) = (tools.is_empty(), self.tool_call_loop.as_ref()) {
            let tool_stream = tcl.stream(
                messages,
                tools,
                prompt_config.temperature,
                prompt_config.max_tokens,
                None,
            );
            stream_response(tool_stream, &self.event_queue, Some(&msg_id)).await
        } else {
            let stream = self.llm.stream(
                messages,
                None,
                prompt_config.response_format,
                prompt_config.temperature,
                prompt_config.max_tokens,
            );
            stream_llm_response(stream, &self.event_queue, Some(&msg_id)).await
        };

        if result.success && !result.full_response.is_empty() {
            self.persist_proactive_response(&result, target_conversation.as_ref())
                .await;
            self.record_engagement().await;
        }

        self.event_queue.set_indicator(IndicatorType::Idle);
    }

    async fn persist_proactive_response(
        &self,
        result: &crate::runtime::response_streamer::StreamResult,
        target: Option<&Conversation>,
    ) {
        let chunks_per_sec = if result.duration_ms > 0.0 {
            result.chunk_count as f64 / (result.duration_ms / 1000.0)
        } else {
            0.0
        };

        if let Some(target) = target {
            match self.conversation.get_conversation(target.id).await {
                Ok(Some(conv)) if !conv.is_closed() => {
                    match self
                        .conversation
                        .add_turn(target.id, TurnRole::Assistant, &result.full_response)
                        .await
                    {
                        Ok(Some(_)) => {
                            info!(
                                chunks = result.chunk_count,
                                ms = result.duration_ms as u64,
                                cps = format!("{chunks_per_sec:.1}"),
                                first_token_ms = ?result.first_token_ms.map(|v| v as u64),
                                "proactive_generator.complete"
                            );
                        }
                        Ok(None) => warn!("proactive_generator.turn_failed"),
                        Err(e) => error!(error = %e, "proactive_generator.conversation_failed"),
                    }
                }
                Ok(Some(_)) => {
                    warn!(
                        conversation_id = %target.id,
                        "proactive_generator.conversation_closed_before_persist"
                    );
                }
                Ok(None) => {
                    error!(conversation_id = %target.id, "proactive_generator.conversation_not_found");
                }
                Err(e) => error!(error = %e, "proactive_generator.conversation_refetch_failed"),
            }
        } else {
            match self
                .conversation
                .create_pending(&result.full_response)
                .await
            {
                Ok(conv) => {
                    info!(
                        conversation_id = &conv.id.to_string()[..8],
                        chunks = result.chunk_count,
                        "proactive_generator.conversation_created"
                    );
                }
                Err(e) => error!(error = %e, "proactive_generator.create_failed"),
            }
        }
    }

    async fn record_engagement(&self) {
        if let Some(ref cooldown_repo) = self.cooldown_repo
            && let Err(e) = cooldown_repo.update_last_engagement(Utc::now()).await
        {
            warn!(error = %e, "proactive_generator.cooldown_update_failed");
        }
    }

    async fn transition_conversation(
        &self,
        old_conversation: &Conversation,
    ) -> (Conversation, Option<String>) {
        let cfg = self.config.load();
        let mut summary: Option<String> = None;

        if cfg.conversation.summary_enabled
            && let Ok(turns) = self
                .conversation
                .get_conversation_turns(old_conversation.id, 50)
                .await
            && turns.len() >= 2
        {
            summary = self.generate_summary(&turns).await;
        }

        match self
            .conversation
            .close_and_start_new(old_conversation.id, summary.clone())
            .await
        {
            Ok(new_conv) => {
                info!(
                    old_id = &old_conversation.id.to_string()[..8],
                    new_id = &new_conv.id.to_string()[..8],
                    had_summary = summary.is_some(),
                    "proactive_generator.conversation_transitioned"
                );
                (new_conv, summary)
            }
            Err(e) => {
                error!(error = %e, "proactive_generator.transition_failed");
                // Return a new pending conversation as fallback
                let fallback = Conversation::new_pending();
                (fallback, None)
            }
        }
    }

    async fn generate_summary(
        &self,
        turns: &[crate::models::conversation::ConversationTurn],
    ) -> Option<String> {
        let turn_tuples: Vec<(String, String)> = turns
            .iter()
            .map(|t| (t.role.as_str().to_owned(), t.content.clone()))
            .collect();

        let turn_refs: Vec<(&str, &str)> = turn_tuples
            .iter()
            .map(|(r, c)| (r.as_str(), c.as_str()))
            .collect();

        let messages = ConversationSummaryPrompt::messages(&turn_refs);
        let prompt_config = ConversationSummaryPrompt::config();

        match tokio::time::timeout(
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
            Ok(Ok(response)) => {
                let content = response.message.content.text_or_empty().trim().to_string();
                if content.is_empty() {
                    None
                } else {
                    Some(content)
                }
            }
            Ok(Err(e)) => {
                warn!(error = %e, "proactive_generator.summary_failed");
                None
            }
            Err(_) => {
                warn!("proactive_generator.summary_timeout");
                None
            }
        }
    }
}
