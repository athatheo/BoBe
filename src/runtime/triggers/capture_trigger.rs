//! Capture trigger — entry point for capture-based proactive engagement.
//!
//! Orchestrates: screenshot → learn → cooldown check → decision → response.

use std::sync::Arc;

use arc_swap::ArcSwap;
use tracing::{debug, error, info};

use crate::config::Config;
use crate::db::CooldownRepository;
use crate::db::ObservationRepository;
use crate::models::observation::Observation;
use crate::runtime::learners::CaptureLearner;
use crate::runtime::learners::types::LearnerObservation;
use crate::runtime::state::{Decision, TriggerContext, TriggerType};
use crate::util::capture::ScreenCapture;
use crate::util::sse::event_queue::EventQueue;
use crate::util::sse::factories::indicator_event;
use crate::util::sse::types::IndicatorType;

use crate::runtime::decision_engine::DecisionEngine;
use crate::runtime::proactive_generator::ProactiveGenerator;

pub struct CaptureTrigger {
    screen_capture: Arc<ScreenCapture>,
    capture_learner: Arc<CaptureLearner>,
    decision_engine: Arc<DecisionEngine>,
    generator: Arc<ProactiveGenerator>,
    cooldown_repo: Option<Arc<dyn CooldownRepository>>,
    observation_repo: Arc<dyn ObservationRepository>,
    event_queue: Arc<EventQueue>,
    config: Arc<ArcSwap<Config>>,
    enabled: bool,
    context_count: usize,
}

impl CaptureTrigger {
    pub fn new(
        screen_capture: Arc<ScreenCapture>,
        capture_learner: Arc<CaptureLearner>,
        decision_engine: Arc<DecisionEngine>,
        generator: Arc<ProactiveGenerator>,
        cooldown_repo: Option<Arc<dyn CooldownRepository>>,
        observation_repo: Arc<dyn ObservationRepository>,
        event_queue: Arc<EventQueue>,
        config: Arc<ArcSwap<Config>>,
    ) -> Self {
        Self {
            screen_capture,
            capture_learner,
            decision_engine,
            generator,
            cooldown_repo,
            observation_repo,
            event_queue,
            config,
            enabled: false,
            context_count: 0,
        }
    }

    pub async fn start(&mut self) {
        self.enabled = true;
        info!("capture_trigger.started");
    }

    pub async fn stop(&mut self) {
        self.enabled = false;
        info!("capture_trigger.stopped");
    }

    /// Execute the capture trigger — takes screenshot internally, learns, then decides.
    pub async fn fire(&mut self) -> Decision {
        let observation = self.run_capture_cycle().await;
        let Some(obs) = observation else {
            return Decision::Idle;
        };

        let cfg = self.config.load();

        // Cooldown check
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

        // Decision
        self.event_queue
            .push(indicator_event(IndicatorType::Thinking, None));
        let context = TriggerContext {
            trigger_type: TriggerType::Capture,
            context_text: obs.content.clone(),
            observation: Some(obs),
            goal: None,
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

    async fn run_capture_cycle(&mut self) -> Option<Observation> {
        let cycle_num = self.context_count + 1;
        info!(cycle = cycle_num, "capture_trigger.cycle_start");

        // 1. Capture screenshot
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

        // 2. Analyze screenshot via learner
        self.event_queue
            .push(indicator_event(IndicatorType::Thinking, None));
        let observation =
            LearnerObservation::capture(capture_result.image, capture_result.active_window);
        match self.capture_learner.learn(&observation).await {
            Ok(result) => {
                self.context_count += 1;
                debug!(cycle = cycle_num, "capture_trigger.cycle_complete");
                self.event_queue
                    .push(indicator_event(IndicatorType::Idle, None));
                match result {
                    crate::runtime::learners::types::LearnerResult::Stored { observation_id } => {
                        match self.observation_repo.get_by_id(observation_id).await {
                            Ok(Some(obs)) => Some(obs),
                            Ok(None) => {
                                debug!("capture_trigger.observation_not_found_after_store");
                                None
                            }
                            Err(e) => {
                                error!(error = %e, "capture_trigger.observation_fetch_failed");
                                None
                            }
                        }
                    }
                    crate::runtime::learners::types::LearnerResult::Skipped { reason } => {
                        debug!(reason = %reason, "capture_trigger.observation_skipped");
                        None
                    }
                }
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
