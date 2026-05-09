//! Capture-based proactive engagement: screenshot → vision-describe →
//! cooldown → decision → response.
//!
//! Pre-pivot the trigger fetched a freshly-stored `Observation` from
//! SQL after the learner ran. Post-pivot the learner returns the
//! description directly (it has already appended a one-liner to
//! memory.md), so we skip the round-trip through `ObservationRepository`.

use std::sync::Arc;

use arc_swap::ArcSwap;
use tracing::{debug, error, info};

use crate::config::Config;
use crate::db::CooldownRepository;
use crate::runtime::decision_engine::DecisionEngine;
use crate::runtime::learners::CaptureLearner;
use crate::runtime::proactive_generator::ProactiveGenerator;
use crate::runtime::state::{Decision, TriggerContext, TriggerType};
use crate::util::capture::ScreenCapture;
use crate::util::sse::event_queue::EventQueue;
use crate::util::sse::factories::indicator_event;
use crate::util::sse::types::IndicatorType;

pub(crate) struct CaptureTrigger {
    screen_capture: Arc<ScreenCapture>,
    capture_learner: Arc<CaptureLearner>,
    decision_engine: Arc<DecisionEngine>,
    generator: Arc<ProactiveGenerator>,
    cooldown_repo: Option<Arc<dyn CooldownRepository>>,
    event_queue: Arc<EventQueue>,
    config: Arc<ArcSwap<Config>>,
    enabled: bool,
    context_count: usize,
}

impl CaptureTrigger {
    pub(crate) fn new(
        screen_capture: Arc<ScreenCapture>,
        capture_learner: Arc<CaptureLearner>,
        decision_engine: Arc<DecisionEngine>,
        generator: Arc<ProactiveGenerator>,
        cooldown_repo: Option<Arc<dyn CooldownRepository>>,
        event_queue: Arc<EventQueue>,
        config: Arc<ArcSwap<Config>>,
    ) -> Self {
        Self {
            screen_capture,
            capture_learner,
            decision_engine,
            generator,
            cooldown_repo,
            event_queue,
            config,
            enabled: false,
            context_count: 0,
        }
    }

    pub(crate) async fn start(&mut self) {
        self.enabled = true;
        info!("capture_trigger.started");
    }

    pub(crate) async fn stop(&mut self) {
        self.enabled = false;
        info!("capture_trigger.stopped");
    }

    pub(crate) async fn fire(&mut self) -> Decision {
        let Some(description) = self.run_capture_cycle().await else {
            return Decision::Idle;
        };

        let cfg = self.config.load();

        if let Some(ref cooldown_repo) = self.cooldown_repo
            && let Some(cooldown) = cooldown_repo.check_cooldown(
                cfg.decision.cooldown_minutes,
                cfg.decision.extended_cooldown_minutes,
            )
        {
            debug!(
                remaining_s = cooldown.remaining.num_seconds(),
                cooldown_type = %cooldown.cooldown_type,
                "capture_trigger.cooldown_active"
            );
            self.event_queue
                .push(indicator_event(IndicatorType::Idle, None));
            return Decision::Idle;
        }

        self.event_queue
            .push(indicator_event(IndicatorType::Thinking, None));
        let context = TriggerContext {
            trigger_type: TriggerType::Capture,
            context_text: description,
        };

        let decision = self.decision_engine.decide(&context).await;
        self.event_queue
            .push(indicator_event(IndicatorType::Idle, None));

        if decision == Decision::Engage {
            self.generator
                .generate_proactive_response(cfg.conversation.auto_close_minutes as i64, None)
                .await;
        }

        decision
    }

    /// Capture the screen, hand the bytes to the learner, return its
    /// description. `None` means the cycle should be treated as a
    /// no-op (capture failed, or the model couldn't say anything
    /// useful).
    async fn run_capture_cycle(&mut self) -> Option<String> {
        let cycle_num = self.context_count + 1;
        info!(cycle = cycle_num, "capture_trigger.cycle_start");

        self.event_queue
            .push(indicator_event(IndicatorType::ScreenCapture, None));
        let capture_result = match self.screen_capture.capture_screen().await {
            Ok(r) => r,
            Err(e) => {
                error!(error = %e, cycle = cycle_num, "capture_trigger.screenshot_failed");
                self.event_queue
                    .push(indicator_event(IndicatorType::Idle, None));
                return None;
            }
        };

        self.event_queue
            .push(indicator_event(IndicatorType::Thinking, None));
        match self
            .capture_learner
            .learn(
                capture_result.image,
                capture_result.active_window.as_deref(),
            )
            .await
        {
            Ok(description) if !description.is_empty() => {
                self.context_count += 1;
                debug!(cycle = cycle_num, "capture_trigger.cycle_complete");
                self.event_queue
                    .push(indicator_event(IndicatorType::Idle, None));
                Some(description)
            }
            Ok(_) => {
                debug!(cycle = cycle_num, "capture_trigger.empty_description");
                self.event_queue
                    .push(indicator_event(IndicatorType::Idle, None));
                None
            }
            Err(e) => {
                error!(error = %e, cycle = cycle_num, "capture_trigger.cycle_failed");
                self.event_queue
                    .push(indicator_event(IndicatorType::Idle, None));
                None
            }
        }
    }
}
