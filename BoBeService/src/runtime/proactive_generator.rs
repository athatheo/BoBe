//! Same SSE pipe as user chat; empty agent response = no-op (not persisted).

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use arc_swap::ArcSwap;
use chrono::Utc;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::config::Config;
use crate::copilot::registry::WorkerRegistry;
use crate::copilot::types::{ChatPrompt, JobInput};
use crate::db::SqliteCooldownRepo;
use crate::error::AppError;
use crate::models::conversation::Conversation;
use crate::models::ids::{ConversationTurnId, message_id_for_turn};
use crate::runtime::behavior_context::BehaviorContext;
use crate::runtime::conversation_service::ConversationService;
use crate::runtime::response_streamer::stream_chat_delta_response;
use crate::runtime::state::Decision;
use crate::util::atomic_flag_guard::AtomicFlagGuard;
use crate::util::sse::event_queue::EventQueue;
use crate::util::sse::factories::{conversation_closed_event, error_event};
use crate::util::sse::indicator_guard::IndicatorGuard;
use crate::util::sse::types::IndicatorType;

const PROACTIVE_TRIGGER_PROMPT: &str = "[bobe.proactive_check] \
     The system has detected an opportunity to engage proactively with the user. \
     Based on the memory.md context and recent conversation, decide whether to \
     produce a short proactive message. If a message would not add value right \
     now, reply with an empty message — no apology, no preamble.";

struct ProactiveStreamGuard {
    conversation: Arc<ConversationService>,
    conversation_id: crate::models::ids::ConversationId,
    armed: bool,
}

impl ProactiveStreamGuard {
    fn new(
        conversation: Arc<ConversationService>,
        conversation_id: crate::models::ids::ConversationId,
    ) -> Self {
        Self {
            conversation,
            conversation_id,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for ProactiveStreamGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }

        let conversation = Arc::clone(&self.conversation);
        let conversation_id = self.conversation_id;
        // Drop cannot await. Settle the draft in one bounded task so visible
        // partial text survives cancellation as an incomplete turn.
        tokio::spawn(async move {
            if let Err(error) = conversation
                .settle_interrupted_proactive_stream(conversation_id)
                .await
            {
                warn!(%error, "proactive_generator.cancel_cleanup_failed");
            }
        });
    }
}

fn inactivity_timeout_elapsed(
    now: chrono::DateTime<Utc>,
    last_user_at: chrono::DateTime<Utc>,
    timeout_seconds: u64,
) -> bool {
    now.signed_duration_since(last_user_at).num_seconds()
        >= i64::try_from(timeout_seconds).unwrap_or(i64::MAX)
}

pub(crate) struct ProactiveGenerator {
    workers: Arc<WorkerRegistry>,
    conversation: Arc<ConversationService>,
    event_queue: Arc<EventQueue>,
    cooldown_repo: Arc<SqliteCooldownRepo>,
    behavior_context: Arc<BehaviorContext>,
    config: Arc<ArcSwap<Config>>,
    in_flight: Arc<AtomicBool>,
}

impl ProactiveGenerator {
    pub(crate) fn new(
        workers: Arc<WorkerRegistry>,
        conversation: Arc<ConversationService>,
        event_queue: Arc<EventQueue>,
        cooldown_repo: Arc<SqliteCooldownRepo>,
        behavior_context: Arc<BehaviorContext>,
        config: Arc<ArcSwap<Config>>,
    ) -> Self {
        Self {
            workers,
            conversation,
            event_queue,
            cooldown_repo,
            behavior_context,
            config,
            in_flight: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) async fn generate_proactive_response(
        &self,
        auto_close_minutes: i64,
        context_summary: Option<String>,
    ) -> Decision {
        match self.conversation.latest_user_turn_at().await {
            Ok(Some(last_user_at))
                if !inactivity_timeout_elapsed(
                    Utc::now(),
                    last_user_at,
                    self.config.load().conversation.inactivity_timeout_seconds,
                ) =>
            {
                tracing::debug!("proactive_generator.user_active");
                return Decision::Idle;
            }
            Ok(_) => {}
            Err(error) => {
                warn!(%error, "proactive_generator.admission_failed");
                return Decision::Idle;
            }
        }

        let Some(_in_flight) = AtomicFlagGuard::try_acquire(Arc::clone(&self.in_flight)) else {
            tracing::debug!("proactive_generator.already_in_flight");
            return Decision::Idle;
        };
        let (target, _previous_summary) = self.ensure_conversation(auto_close_minutes).await;
        let (target_conversation, assistant_turn_id) = match self
            .conversation
            .begin_proactive_stream(target.as_ref())
            .await
        {
            Ok(started) => started,
            Err(e) => {
                error!(error = %e, "proactive_generator.begin_stream_failed");
                return Decision::Idle;
            }
        };

        self.generate_response(&target_conversation, assistant_turn_id, context_summary)
            .await
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
            let last_user_at = self
                .conversation
                .last_user_turn_at(conv.id)
                .await
                .ok()
                .flatten();
            if conv.is_stale_since(auto_close_minutes, last_user_at) {
                let old_turn_count = self
                    .conversation
                    .get_conversation_turns(conv.id, 100)
                    .await
                    .map_or(0, |turns| turns.len() as u32);
                let old_id = conv.id.to_string();
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
        assistant_turn_id: ConversationTurnId,
        context_summary: Option<String>,
    ) -> Decision {
        let mut stream_guard =
            ProactiveStreamGuard::new(Arc::clone(&self.conversation), target_conversation.id);
        self.event_queue.set_indicator(IndicatorType::Streaming);
        // RAII: covers normal completion, error return, AND mid-flight
        // task abort (e.g., shutdown). Without this guard a panic or
        // abort during send_proactive_via_chat / persist_proactive_response
        // would leave the indicator stuck Streaming and reject every
        // subsequent turn. Mirror of the message_handler.rs pattern.
        let _indicator_guard = IndicatorGuard::new(Arc::clone(&self.event_queue));
        let msg_id = message_id_for_turn(assistant_turn_id);
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
                let cleanup = self
                    .conversation
                    .discard_proactive_stream(conversation_id)
                    .await;
                if let Err(cleanup_error) = cleanup {
                    warn!(
                        error = %cleanup_error,
                        "proactive_generator.discard_failed"
                    );
                } else {
                    stream_guard.disarm();
                }
                return Decision::Idle;
            }
        };

        let decision = if result.full_response.trim().is_empty() {
            let cleanup = self
                .conversation
                .discard_proactive_stream(conversation_id)
                .await;
            if let Err(error) = cleanup {
                warn!(%error, "proactive_generator.discard_failed");
            } else {
                stream_guard.disarm();
            }
            Decision::Idle
        } else {
            let persisted = self
                .persist_proactive_response(&result, target_conversation)
                .await;
            if persisted {
                stream_guard.disarm();
            }
            if result.success && persisted {
                result.emit_terminal(
                    &self.event_queue,
                    crate::runtime::response_streamer::StreamDelivery::LegacySse,
                );
                self.record_engagement().await;
            } else if result.success {
                self.event_queue.push(error_event(
                    &msg_id,
                    "RESPONSE_PERSIST_FAILED",
                    "BoBe could not save this proactive response. It may not survive a restart.",
                    true,
                ));
            }
            Decision::Engage
        };

        if !result.success {
            warn!(
                conversation_id = %conversation_id,
                "proactive_generator.stream_incomplete"
            );
        }
        decision
    }

    async fn send_proactive_via_chat(
        &self,
        prompt_text: &str,
        msg_id: &str,
        conversation_id: crate::models::ids::ConversationId,
    ) -> Result<crate::runtime::response_streamer::StreamResult, AppError> {
        let worker = self.workers.chat().await?;
        let prompt_text = self.behavior_context.prepend_to(prompt_text).await;
        let chat_stream = worker
            .send(ChatPrompt::text(prompt_text))
            .await
            .map_err(|e| AppError::Internal(format!("chat_worker.send: {e}")))?;
        info!(msg_id, "proactive_generator.stream_start");
        let conversation = Arc::clone(&self.conversation);
        Ok(stream_chat_delta_response(
            chat_stream,
            &self.event_queue,
            Some(msg_id),
            crate::runtime::response_streamer::StreamDelivery::LegacySse,
            move |delta| {
                let conversation = Arc::clone(&conversation);
                async move {
                    conversation.push_proactive_stream_delta(conversation_id, &delta);
                }
            },
        )
        .await)
    }

    async fn persist_proactive_response(
        &self,
        result: &crate::runtime::response_streamer::StreamResult,
        target: &Conversation,
    ) -> bool {
        match self
            .conversation
            .finalize_proactive_stream(target.id, &result.full_response, result.success)
            .await
        {
            Ok(Some(_)) => {
                info!(
                    chunks = result.chunk_count,
                    ms = result.duration_ms as u64,
                    cps = format!("{:.1}", result.chunks_per_sec()),
                    first_token_ms = ?result.first_token_ms.map(|v| v as u64),
                    "proactive_generator.complete"
                );
                true
            }
            Ok(None) => {
                warn!(
                    conversation_id = %target.id,
                    "proactive_generator.turn_finalize_skipped"
                );
                false
            }
            Err(e) => {
                error!(error = %e, "proactive_generator.conversation_failed");
                false
            }
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration as StdDuration;

    use super::{ProactiveStreamGuard, inactivity_timeout_elapsed};
    use crate::db::{SqliteConversationRepo, test_helpers::in_memory_pool};
    use crate::runtime::conversation_service::ConversationService;
    use chrono::{Duration, Utc};

    #[test]
    fn inactivity_timeout_blocks_recent_user_response() {
        let now = Utc::now();
        assert!(!inactivity_timeout_elapsed(
            now,
            now - Duration::seconds(29),
            30
        ));
    }

    #[test]
    fn inactivity_timeout_allows_at_boundary() {
        let now = Utc::now();
        assert!(inactivity_timeout_elapsed(
            now,
            now - Duration::seconds(30),
            30
        ));
    }

    #[test]
    fn inactivity_timeout_blocks_future_user_response() {
        let now = Utc::now();
        assert!(!inactivity_timeout_elapsed(
            now,
            now + Duration::seconds(1),
            30
        ));
    }

    #[tokio::test]
    async fn dropped_stream_guard_discards_draft_and_empty_placeholder() {
        let repo = Arc::new(SqliteConversationRepo::new(in_memory_pool().await));
        let conversation = Arc::new(ConversationService::new(Arc::clone(&repo)));
        let result = conversation.begin_proactive_stream(None).await;
        let Ok((pending, _)) = result else {
            panic!("proactive stream should start");
        };

        drop(ProactiveStreamGuard::new(
            Arc::clone(&conversation),
            pending.id,
        ));

        let deleted = tokio::time::timeout(StdDuration::from_secs(1), async {
            loop {
                match repo.get_by_id(pending.id).await {
                    Ok(None) => return true,
                    Ok(Some(_)) => tokio::task::yield_now().await,
                    Err(_) => return false,
                }
            }
        })
        .await
        .unwrap_or(false);
        assert!(deleted);
    }
}
