//! Checkin trigger — entry point for scheduled check-ins.
//!
//! Orchestrates: check schedule → active conversation check → cooldown → send.

use std::sync::Arc;

use arc_swap::ArcSwap;
use chrono::{DateTime, Utc};
use tracing::{debug, info};

use crate::config::Config;
use crate::db::CooldownRepository;
use crate::runtime::proactive_generator::ProactiveGenerator;
use crate::runtime::state::Decision;
use crate::runtime::triggers::checkin_scheduler::CheckinScheduler;
use crate::services::conversation_service::ConversationService;

pub struct CheckinTrigger {
    scheduler: CheckinScheduler,
    generator: Arc<ProactiveGenerator>,
    conversation: Arc<ConversationService>,
    cooldown_repo: Option<Arc<dyn CooldownRepository>>,
    config: Arc<ArcSwap<Config>>,
}

impl CheckinTrigger {
    pub fn new(
        scheduler: CheckinScheduler,
        generator: Arc<ProactiveGenerator>,
        conversation: Arc<ConversationService>,
        cooldown_repo: Option<Arc<dyn CooldownRepository>>,
        config: Arc<ArcSwap<Config>>,
    ) -> Self {
        Self {
            scheduler,
            generator,
            conversation,
            cooldown_repo,
            config,
        }
    }

    /// Execute the checkin trigger. Returns `Decision::Engage` if check-in was sent.
    pub async fn fire(&mut self) -> Decision {
        if !self.scheduler.should_checkin() {
            return Decision::Idle;
        }

        let cfg = self.config.load();

        // Skip if active non-stale conversation
        if let Ok(Some(existing)) = self.conversation.get_pending_or_active().await {
            // Get turns to check staleness
            if let Ok(turns) = self
                .conversation
                .get_conversation_turns(existing.id, 100)
                .await
                && !existing.is_stale(cfg.conversation_auto_close_minutes as i64, &turns)
            {
                debug!(reason = "active_conversation", "checkin_trigger.skipped");
                self.scheduler.mark_checkin_done();
                return Decision::Idle;
            }
        }

        // Cooldown check
        if let Some(ref cooldown_repo) = self.cooldown_repo
            && let Some(cooldown) = cooldown_repo.check_cooldown(
                cfg.decision_cooldown_minutes,
                cfg.decision_extended_cooldown_minutes,
            )
        {
            debug!(
                remaining_s = cooldown.remaining.num_seconds(),
                cooldown_type = %cooldown.cooldown_type,
                "checkin_trigger.cooldown_skipped"
            );
            self.scheduler.mark_checkin_done();
            return Decision::Idle;
        }

        // Generate LLM-powered check-in
        info!("checkin_trigger.started");
        self.generator
            .generate_proactive_response(
                cfg.conversation_auto_close_minutes as i64,
                Some("Scheduled check-in".into()),
            )
            .await;
        self.scheduler.mark_checkin_done();
        info!("checkin_trigger.complete");

        Decision::Engage
    }

    pub fn get_next_checkin_time(&mut self) -> Option<DateTime<Utc>> {
        self.scheduler.get_next_checkin_time()
    }
}
