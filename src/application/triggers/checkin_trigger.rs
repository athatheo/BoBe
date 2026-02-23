//! Checkin trigger — entry point for scheduled check-ins.
//!
//! Orchestrates: check schedule → active conversation check → cooldown → send.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use tracing::{debug, info};

use crate::application::runtime::proactive_generator::ProactiveGenerator;
use crate::application::runtime::state::{Decision, OrchestratorConfig};
use crate::application::services::conversation_service::ConversationService;
use crate::application::triggers::checkin_scheduler::CheckinScheduler;
use crate::ports::repos::cooldown_repo::CooldownRepository;

pub struct CheckinTrigger {
    scheduler: CheckinScheduler,
    generator: Arc<ProactiveGenerator>,
    conversation: Arc<ConversationService>,
    cooldown_repo: Option<Arc<dyn CooldownRepository>>,
    config: OrchestratorConfig,
}

impl CheckinTrigger {
    pub fn new(
        scheduler: CheckinScheduler,
        generator: Arc<ProactiveGenerator>,
        conversation: Arc<ConversationService>,
        cooldown_repo: Option<Arc<dyn CooldownRepository>>,
        config: OrchestratorConfig,
    ) -> Self {
        Self {
            scheduler,
            generator,
            conversation,
            cooldown_repo,
            config,
        }
    }

    pub fn update_config(&mut self, config: OrchestratorConfig) {
        self.config = config;
    }

    pub fn is_scheduled(&mut self) -> bool {
        self.scheduler.should_checkin()
    }

    /// Execute the checkin trigger. Returns `Decision::Engage` if check-in was sent.
    pub async fn fire(&mut self) -> Decision {
        if !self.scheduler.should_checkin() {
            return Decision::Idle;
        }

        // Skip if active non-stale conversation
        if let Ok(Some(existing)) = self.conversation.get_pending_or_active().await {
            // Get turns to check staleness
            if let Ok(turns) = self
                .conversation
                .get_conversation_turns(existing.id, 100)
                .await
                && !existing.is_stale(self.config.conversation_auto_close_minutes as i64, &turns)
            {
                debug!(reason = "active_conversation", "checkin_trigger.skipped");
                self.scheduler.mark_checkin_done();
                return Decision::Idle;
            }
        }

        // Cooldown check
        if let Some(ref cooldown_repo) = self.cooldown_repo
            && let Some(cooldown) = cooldown_repo.check_cooldown(
                self.config.decision_cooldown_minutes,
                self.config.decision_extended_cooldown_minutes,
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
                self.config.conversation_auto_close_minutes as i64,
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
