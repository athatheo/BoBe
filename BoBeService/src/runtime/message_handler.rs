//! Handles incoming user messages: conversation lifecycle, ChatWorker
//! streaming, persist response.
//!
//! Pre-pivot this file built a system+history+memory prompt itself and
//! drove an `LlmProvider::stream` (with optional `tool_call_loop`).
//! Post-pivot the SDK owns: per-turn context (memory.md injected via
//! `BobeHooks::SessionStart`), conversation history (Copilot session +
//! `InfiniteSessionConfig` auto-compaction), tool dispatch (Copilot's
//! built-in tools), and streaming (`ChatWorker::send` returns
//! `Stream<ChatDelta>`). Our job here shrinks to: append the user
//! turn, drive the chat stream into SSE, persist the final assistant
//! turn into the local conversation log.

use std::sync::Arc;

use chrono::Utc;
use tracing::{error, info, warn};

use crate::copilot::registry::WorkerRegistry;
use crate::copilot::types::ChatPrompt;
use crate::copilot::workers::ChatWorker;
use crate::db::CooldownRepository;
use crate::error::AppError;
use crate::models::ids::ConversationId;
use crate::models::types::TurnRole;
use crate::runtime::response_streamer::stream_chat_delta_response;
use crate::services::conversation_service::ConversationService;
use crate::util::sse::event_queue::EventQueue;
use crate::util::sse::types::IndicatorType;

pub(crate) struct MessageHandler {
    workers: Arc<WorkerRegistry>,
    conversation: Arc<ConversationService>,
    cooldown_repo: Option<Arc<dyn CooldownRepository>>,
    event_queue: Arc<EventQueue>,
}

impl MessageHandler {
    pub(crate) fn new(
        workers: Arc<WorkerRegistry>,
        conversation: Arc<ConversationService>,
        cooldown_repo: Option<Arc<dyn CooldownRepository>>,
        event_queue: Arc<EventQueue>,
    ) -> Self {
        Self {
            workers,
            conversation,
            cooldown_repo,
            event_queue,
        }
    }

    pub(crate) async fn handle_message(&self, content: &str, message_id: &str) {
        if let Some(ref cooldown_repo) = self.cooldown_repo
            && let Err(e) = cooldown_repo.update_last_user_response(Utc::now()).await
        {
            warn!(error = %e, "message_handler.cooldown_update_failed");
        }

        let conversation_id = self.ensure_active_conversation(content).await;
        let Some(conversation_id) = conversation_id else {
            error!("message_handler.conversation_failed");
            return;
        };

        // User message lives in the local conversation log AND in the
        // Copilot session (via `chat_worker.send`). Memory distillation
        // happens in the nightly consolidation worker reading chat
        // history — no parallel SQL observation insert.
        self.respond_to_message(message_id, content, conversation_id)
            .await;
    }

    async fn ensure_active_conversation(&self, user_content: &str) -> Option<ConversationId> {
        match self
            .conversation
            .append_user_turn_or_create_active(user_content)
            .await
        {
            Ok(conversation) => Some(conversation.id),
            Err(e) => {
                error!(error = %e, "message_handler.create_conversation_failed");
                None
            }
        }
    }

    async fn respond_to_message(
        &self,
        msg_id: &str,
        user_content: &str,
        conversation_id: ConversationId,
    ) {
        self.event_queue.set_indicator(IndicatorType::Streaming);

        let result = match self.send_via_chat_worker(user_content, msg_id).await {
            Ok(r) => r,
            Err(e) => {
                error!(error = %e, "message_handler.chat_worker_failed");
                self.event_queue.set_indicator(IndicatorType::Idle);
                return;
            }
        };

        self.persist_response(&result, conversation_id).await;
        self.event_queue.set_indicator(IndicatorType::Idle);
    }

    /// Lazy-spawn the chat worker (idempotent — registry caches it),
    /// send the user turn, pump the resulting `Stream<ChatDelta>` into
    /// the SSE event queue.
    async fn send_via_chat_worker(
        &self,
        user_content: &str,
        msg_id: &str,
    ) -> Result<crate::runtime::response_streamer::StreamResult, AppError> {
        let worker = self.workers.chat().await?;
        let chat_stream = worker
            .send(ChatPrompt::text(user_content))
            .await
            .map_err(|e| AppError::Internal(format!("chat_worker.send: {e}")))?;
        info!(msg_id, "message_handler.stream_start");
        Ok(stream_chat_delta_response(chat_stream, &self.event_queue, Some(msg_id), |_| {}).await)
    }

    async fn persist_response(
        &self,
        result: &crate::runtime::response_streamer::StreamResult,
        conversation_id: ConversationId,
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
                error!(
                    conversation_id = %conversation_id,
                    "message_handler.conversation_not_found"
                );
            }
            Err(e) => {
                error!(error = %e, "message_handler.conversation_refetch_failed");
            }
        }
    }
}
