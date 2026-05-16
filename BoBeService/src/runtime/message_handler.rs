//! We persist user turn + final assistant turn; SDK owns context/history/tools.

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
use crate::runtime::conversation_service::ConversationService;
use crate::runtime::response_streamer::stream_chat_delta_response;
use crate::util::sse::event_queue::EventQueue;
use crate::util::sse::indicator_guard::IndicatorGuard;
use crate::util::sse::types::IndicatorType;

pub(crate) struct MessageHandler {
    workers: Arc<WorkerRegistry>,
    conversation: Arc<ConversationService>,
    cooldown_repo: Arc<dyn CooldownRepository>,
    event_queue: Arc<EventQueue>,
}

impl MessageHandler {
    pub(crate) fn new(
        workers: Arc<WorkerRegistry>,
        conversation: Arc<ConversationService>,
        cooldown_repo: Arc<dyn CooldownRepository>,
        event_queue: Arc<EventQueue>,
    ) -> Self {
        Self {
            workers,
            conversation,
            cooldown_repo,
            event_queue,
        }
    }

    /// Default text-chat entry point. The observer is a no-op so SSE deltas
    /// are the only consumer of token text.
    pub(crate) async fn handle_message(&self, content: &str, message_id: &str) {
        self.handle_message_with_observer(content, message_id, false, |_: &str| {})
            .await;
    }

    /// Variant that lets the caller subscribe to text deltas in addition to
    /// SSE delivery. Voice uses this to pipe the same tokens into a sentence
    /// buffer for Kokoro TTS and passes `voice_mode=true` so the SDK send
    /// rides `DeliveryMode::Immediate` (atomic server-side interrupt).
    pub(crate) async fn handle_message_with_observer<F>(
        &self,
        content: &str,
        message_id: &str,
        voice_mode: bool,
        on_text_delta: F,
    ) where
        F: FnMut(&str) + Send,
    {
        if let Err(e) = self
            .cooldown_repo
            .update_last_user_response(Utc::now())
            .await
        {
            warn!(error = %e, "message_handler.cooldown_update_failed");
        }

        let conversation_id = self.ensure_active_conversation(content).await;
        let Some(conversation_id) = conversation_id else {
            error!("message_handler.conversation_failed");
            return;
        };

        self.respond_to_message(
            message_id,
            content,
            conversation_id,
            voice_mode,
            on_text_delta,
        )
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

    async fn respond_to_message<F>(
        &self,
        msg_id: &str,
        user_content: &str,
        conversation_id: ConversationId,
        voice_mode: bool,
        on_text_delta: F,
    ) where
        F: FnMut(&str) + Send,
    {
        self.event_queue.set_indicator(IndicatorType::Streaming);
        // RAII: ensures Indicator returns to Idle on every exit path —
        // normal completion, error return, AND mid-flight task abort
        // (voice WS disconnect cancels the per-turn task; explicit
        // set_indicator(Idle) below would otherwise be skipped, leaving
        // the indicator stuck Streaming and rejecting all future turns).
        let _indicator_guard = IndicatorGuard::new(Arc::clone(&self.event_queue));

        let result = match self
            .send_via_chat_worker(user_content, msg_id, voice_mode, on_text_delta)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                error!(error = %e, "message_handler.chat_worker_failed");
                return;
            }
        };

        self.persist_response(&result, conversation_id).await;
    }

    async fn send_via_chat_worker<F>(
        &self,
        user_content: &str,
        msg_id: &str,
        voice_mode: bool,
        on_text_delta: F,
    ) -> Result<crate::runtime::response_streamer::StreamResult, AppError>
    where
        F: FnMut(&str) + Send,
    {
        let worker = self.workers.chat().await?;
        let prompt = if voice_mode {
            ChatPrompt::voice(user_content)
        } else {
            ChatPrompt::text(user_content)
        };
        let chat_stream = worker
            .send(prompt)
            .await
            .map_err(|e| AppError::Internal(format!("chat_worker.send: {e}")))?;
        info!(msg_id, voice_mode, "message_handler.stream_start");
        Ok(
            stream_chat_delta_response(chat_stream, &self.event_queue, Some(msg_id), on_text_delta)
                .await,
        )
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
