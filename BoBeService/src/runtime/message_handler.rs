//! We persist user turn + final assistant turn; SDK owns context/history/tools.

use std::future::Future;
use std::sync::Arc;

use chrono::Utc;
use tracing::{error, info, warn};

use crate::copilot::registry::WorkerRegistry;
use crate::copilot::types::ChatPrompt;
use crate::db::SqliteCooldownRepo;
use crate::error::AppError;
use crate::models::ids::{ConversationId, ConversationTurnId};
use crate::models::types::TurnRole;
use crate::runtime::behavior_context::BehaviorContext;
use crate::runtime::conversation_service::ConversationService;
use crate::runtime::response_streamer::{StreamDelivery, stream_chat_delta_response};
use crate::util::sse::event_queue::EventQueue;
use crate::util::sse::factories::error_event;
use crate::util::sse::indicator_guard::IndicatorGuard;
use crate::util::sse::types::IndicatorType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResponsePersistence {
    Persisted,
    EmptyNoop,
    Failed,
}

struct ConversationChangeGuard {
    event_queue: Arc<EventQueue>,
    conversation_id: ConversationId,
    enabled: bool,
}

impl ConversationChangeGuard {
    fn new(
        event_queue: Arc<EventQueue>,
        conversation_id: ConversationId,
        delivery: StreamDelivery,
    ) -> Self {
        Self {
            event_queue,
            conversation_id,
            enabled: delivery.is_endpoint_only(),
        }
    }
}

impl Drop for ConversationChangeGuard {
    fn drop(&mut self) {
        if self.enabled {
            self.event_queue
                .push(crate::util::sse::factories::conversation_changed_event(
                    &self.conversation_id.to_string(),
                ));
        }
    }
}

pub(crate) struct MessageHandler {
    workers: Arc<WorkerRegistry>,
    conversation: Arc<ConversationService>,
    cooldown_repo: Arc<SqliteCooldownRepo>,
    event_queue: Arc<EventQueue>,
    behavior_context: Arc<BehaviorContext>,
}

impl MessageHandler {
    pub(crate) fn new(
        workers: Arc<WorkerRegistry>,
        conversation: Arc<ConversationService>,
        cooldown_repo: Arc<SqliteCooldownRepo>,
        event_queue: Arc<EventQueue>,
        behavior_context: Arc<BehaviorContext>,
    ) -> Self {
        Self {
            workers,
            conversation,
            cooldown_repo,
            event_queue,
            behavior_context,
        }
    }

    /// Default text-chat entry point. The observer is a no-op so SSE deltas
    /// are the only consumer of token text.
    pub(crate) async fn handle_message(
        &self,
        content: &str,
        message_id: &str,
        assistant_turn_id: ConversationTurnId,
    ) -> bool {
        self.handle_message_with_observer(
            content,
            message_id,
            assistant_turn_id,
            false,
            StreamDelivery::LegacySse,
            |_| async {},
        )
        .await
    }

    /// Variant that lets the caller subscribe to text deltas in addition to
    /// SSE delivery. Voice uses this to pipe the same tokens into a sentence
    /// buffer for Kokoro TTS and passes `voice_mode=true` so the SDK send
    /// rides `DeliveryMode::Immediate` (atomic server-side interrupt).
    pub(crate) async fn handle_message_with_observer<F, Fut>(
        &self,
        content: &str,
        message_id: &str,
        assistant_turn_id: ConversationTurnId,
        voice_mode: bool,
        delivery: StreamDelivery,
        on_text_delta: F,
    ) -> bool
    where
        F: FnMut(String) -> Fut + Send,
        Fut: Future<Output = ()> + Send,
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
            return false;
        };
        let _conversation_change =
            ConversationChangeGuard::new(Arc::clone(&self.event_queue), conversation_id, delivery);

        self.respond_to_message(
            message_id,
            assistant_turn_id,
            content,
            conversation_id,
            voice_mode,
            delivery,
            on_text_delta,
        )
        .await
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

    async fn respond_to_message<F, Fut>(
        &self,
        msg_id: &str,
        assistant_turn_id: ConversationTurnId,
        user_content: &str,
        conversation_id: ConversationId,
        voice_mode: bool,
        delivery: StreamDelivery,
        on_text_delta: F,
    ) -> bool
    where
        F: FnMut(String) -> Fut + Send,
        Fut: Future<Output = ()> + Send,
    {
        self.event_queue.set_indicator(IndicatorType::Streaming);
        // RAII: ensures Indicator returns to Idle on every exit path —
        // normal completion, error return, AND mid-flight task abort
        // (voice WS disconnect cancels the per-turn task; explicit
        // set_indicator(Idle) below would otherwise be skipped, leaving
        // the indicator stuck Streaming and rejecting all future turns).
        let _indicator_guard = IndicatorGuard::new(Arc::clone(&self.event_queue));

        let result = match self
            .send_via_chat_worker(user_content, msg_id, voice_mode, delivery, on_text_delta)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                error!(error = %e, "message_handler.chat_worker_failed");
                return false;
            }
        };

        let persistence = self
            .persist_response(&result, conversation_id, assistant_turn_id)
            .await;
        if result.success {
            match persistence {
                ResponsePersistence::Persisted | ResponsePersistence::EmptyNoop => {
                    result.emit_terminal(&self.event_queue, delivery);
                    return true;
                }
                ResponsePersistence::Failed if delivery.emits_legacy_sse() => {
                    self.event_queue.push(error_event(
                        msg_id,
                        "RESPONSE_PERSIST_FAILED",
                        "BoBe could not save this response. It may not survive a restart.",
                        true,
                    ));
                }
                ResponsePersistence::Failed => {}
            }
        }
        false
    }

    async fn send_via_chat_worker<F, Fut>(
        &self,
        user_content: &str,
        msg_id: &str,
        voice_mode: bool,
        delivery: StreamDelivery,
        on_text_delta: F,
    ) -> Result<crate::runtime::response_streamer::StreamResult, AppError>
    where
        F: FnMut(String) -> Fut + Send,
        Fut: Future<Output = ()> + Send,
    {
        let worker = self.workers.chat().await?;
        let contextualized = self.behavior_context.prepend_to(user_content).await;
        let prompt = if voice_mode {
            ChatPrompt::voice(contextualized)
        } else {
            ChatPrompt::text(contextualized)
        };
        let chat_stream = worker
            .send(prompt)
            .await
            .map_err(|e| AppError::Internal(format!("chat_worker.send: {e}")))?;
        info!(msg_id, voice_mode, "message_handler.stream_start");
        Ok(stream_chat_delta_response(
            chat_stream,
            &self.event_queue,
            Some(msg_id),
            delivery,
            on_text_delta,
        )
        .await)
    }

    async fn persist_response(
        &self,
        result: &crate::runtime::response_streamer::StreamResult,
        conversation_id: ConversationId,
        assistant_turn_id: ConversationTurnId,
    ) -> ResponsePersistence {
        if result.full_response.is_empty() {
            info!(
                conversation_id = %conversation_id,
                "message_handler.empty_response_completed"
            );
            return ResponsePersistence::EmptyNoop;
        }

        match self
            .conversation
            .add_turn_with_id(
                assistant_turn_id,
                conversation_id,
                TurnRole::Assistant,
                &result.full_response,
                result.success,
            )
            .await
        {
            Ok(Some(_)) => {
                info!(
                    chunks = result.chunk_count,
                    ms = result.duration_ms as u64,
                    cps = format!("{:.1}", result.chunks_per_sec()),
                    complete = result.success,
                    "message_handler.response_persisted"
                );
                ResponsePersistence::Persisted
            }
            Ok(None) => {
                error!(
                    conversation_id = %conversation_id,
                    "message_handler.conversation_not_found"
                );
                ResponsePersistence::Failed
            }
            Err(e) => {
                error!(error = %e, "message_handler.persist_failed");
                ResponsePersistence::Failed
            }
        }
    }
}
