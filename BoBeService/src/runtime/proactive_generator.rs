//! Same SSE pipe as user chat; empty agent response = no-op (not persisted).

use std::sync::Arc;

use chrono::Utc;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::copilot::registry::WorkerRegistry;
use crate::copilot::types::{ChatPrompt, JobInput};
use crate::copilot::workers::ChatWorker;
use crate::db::CooldownRepository;
use crate::error::AppError;
use crate::models::conversation::Conversation;
use crate::runtime::response_streamer::stream_chat_delta_response;
use crate::services::conversation_service::ConversationService;
use crate::util::sse::event_queue::EventQueue;
use crate::util::sse::factories::conversation_closed_event;
use crate::util::sse::types::IndicatorType;

const PROACTIVE_TRIGGER_PROMPT: &str = "[bobe.proactive_check] \
     The system has detected an opportunity to engage proactively with the user. \
     Based on the memory.md context and recent conversation, decide whether to \
     produce a short proactive message. If a message would not add value right \
     now, reply with an empty message — no apology, no preamble.";

pub(crate) struct ProactiveGenerator {
    workers: Arc<WorkerRegistry>,
    conversation: Arc<ConversationService>,
    event_queue: Arc<EventQueue>,
    cooldown_repo: Arc<dyn CooldownRepository>,
}

impl ProactiveGenerator {
    pub(crate) fn new(
        workers: Arc<WorkerRegistry>,
        conversation: Arc<ConversationService>,
        event_queue: Arc<EventQueue>,
        cooldown_repo: Arc<dyn CooldownRepository>,
    ) -> Self {
        Self {
            workers,
            conversation,
            event_queue,
            cooldown_repo,
        }
    }

    pub(crate) async fn generate_proactive_response(
        &self,
        auto_close_minutes: i64,
        context_summary: Option<String>,
    ) {
        let (target, _previous_summary) = self.ensure_conversation(auto_close_minutes).await;
        let target = match self
            .conversation
            .begin_proactive_stream(target.as_ref())
            .await
        {
            Ok(target) => Some(target),
            Err(e) => {
                error!(error = %e, "proactive_generator.begin_stream_failed");
                target
            }
        };

        let Some(target_conversation) = target else {
            warn!("proactive_generator.missing_target_conversation");
            return;
        };

        self.generate_response(&target_conversation, context_summary)
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
        target_conversation: &Conversation,
        context_summary: Option<String>,
    ) {
        self.event_queue.set_indicator(IndicatorType::Streaming);
        let msg_id = format!("msg_{}", Uuid::new_v4().simple());
        let conversation_id = target_conversation.id;

        let prompt_text = match context_summary.as_deref() {
            Some(cs) if !cs.is_empty() => {
                format!("{PROACTIVE_TRIGGER_PROMPT}\n\nrecent_context = {cs}")
            }
            _ => PROACTIVE_TRIGGER_PROMPT.to_string(),
        };

        let result = match self
            .send_proactive_via_chat(&prompt_text, &msg_id, conversation_id)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                error!(error = %e, "proactive_generator.chat_failed");
                self.conversation.discard_proactive_stream(conversation_id);
                self.event_queue.set_indicator(IndicatorType::Idle);
                return;
            }
        };

        if result.full_response.trim().is_empty() {
            self.conversation.discard_proactive_stream(conversation_id);
        } else {
            self.persist_proactive_response(&result, target_conversation)
                .await;
            if result.success {
                self.record_engagement().await;
            }
        }

        if !result.success {
            warn!(
                conversation_id = %conversation_id,
                "proactive_generator.stream_incomplete"
            );
        }

        self.event_queue.set_indicator(IndicatorType::Idle);
    }

    async fn send_proactive_via_chat(
        &self,
        prompt_text: &str,
        msg_id: &str,
        conversation_id: crate::models::ids::ConversationId,
    ) -> Result<crate::runtime::response_streamer::StreamResult, AppError> {
        let worker = self.workers.chat().await?;
        let chat_stream = worker
            .send(ChatPrompt::text(prompt_text))
            .await
            .map_err(|e| AppError::Internal(format!("chat_worker.send: {e}")))?;
        info!(msg_id, "proactive_generator.stream_start");
        let conversation = Arc::clone(&self.conversation);
        Ok(
            stream_chat_delta_response(
                chat_stream,
                &self.event_queue,
                Some(msg_id),
                move |delta| {
                    conversation.push_proactive_stream_delta(conversation_id, delta);
                },
            )
            .await,
        )
    }

    async fn persist_proactive_response(
        &self,
        result: &crate::runtime::response_streamer::StreamResult,
        target: &Conversation,
    ) {
        let chunks_per_sec = if result.duration_ms > 0.0 {
            result.chunk_count as f64 / (result.duration_ms / 1000.0)
        } else {
            0.0
        };

        match self
            .conversation
            .finalize_proactive_stream(target.id, &result.full_response)
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
            Ok(None) => {
                warn!(
                    conversation_id = %target.id,
                    "proactive_generator.turn_finalize_skipped"
                );
            }
            Err(e) => error!(error = %e, "proactive_generator.conversation_failed"),
        }
    }

    async fn record_engagement(&self) {
        if let Err(e) = self.cooldown_repo.update_last_engagement(Utc::now()).await {
            warn!(error = %e, "proactive_generator.cooldown_update_failed");
        }
    }

    async fn transition_conversation(
        &self,
        old_conversation: &Conversation,
    ) -> (Conversation, Option<String>) {
        let mut summary: Option<String> = None;

        if let Ok(turns) = self
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
                let fallback = Conversation::new_pending();
                (fallback, None)
            }
        }
    }

    async fn generate_summary(
        &self,
        turns: &[crate::models::conversation::ConversationTurn],
    ) -> Option<String> {
        let transcript = turns
            .iter()
            .map(|t| format!("{}: {}", t.role.as_str(), t.content))
            .collect::<Vec<_>>()
            .join("\n");

        let worker = match self.workers.goals().await {
            Ok(w) => w,
            Err(e) => {
                warn!(err = %e, "proactive_generator.summary.worker_unavailable");
                return None;
            }
        };

        let job = JobInput {
            job_id: Uuid::new_v4(),
            kind: "conversation_summary".into(),
            instructions: "Summarize the conversation in 1-2 sentences. Capture the gist, \
                 not the details. Return JSON {\"output\":\"<summary>\"}."
                .into(),
            input: serde_json::json!({ "transcript": transcript }),
        };

        match worker.submit(job).await {
            Ok(out) => {
                let summary = out.output.as_str().unwrap_or("").trim().to_string();
                if summary.is_empty() {
                    None
                } else {
                    Some(summary)
                }
            }
            Err(e) => {
                warn!(error = %e, "proactive_generator.summary_failed");
                None
            }
        }
    }
}
