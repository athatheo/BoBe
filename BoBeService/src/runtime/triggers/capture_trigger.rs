use std::sync::Arc;
use std::time::{Duration, Instant};

use arc_swap::ArcSwap;
use tracing::{debug, error, info, warn};

const VISION_FAILURE_BREAKER_THRESHOLD: u32 = 3;

const VISION_FAILURE_COOLDOWN: Duration = Duration::from_mins(3);

use crate::config::Config;
use crate::db::CooldownRepository;
use crate::runtime::decision_engine::DecisionEngine;
use crate::runtime::learners::CaptureLearner;
use crate::runtime::proactive_generator::ProactiveGenerator;
use crate::runtime::state::{Decision, TriggerContext, TriggerType};
use crate::util::capture::ScreenCapture;
use crate::util::sse::event_queue::EventQueue;
use crate::util::sse::factories::{indicator_event, trigger_error_event};
use crate::util::sse::types::IndicatorType;

pub(crate) struct CaptureTrigger {
    screen_capture: Arc<ScreenCapture>,
    capture_learner: Arc<CaptureLearner>,
    decision_engine: Arc<DecisionEngine>,
    generator: Arc<ProactiveGenerator>,
    cooldown_repo: Arc<dyn CooldownRepository>,
    event_queue: Arc<EventQueue>,
    config: Arc<ArcSwap<Config>>,
    enabled: bool,
    context_count: usize,
    vision_failure_count: u32,
    vision_breaker_tripped_at: Option<Instant>,
    /// Suppresses alternating paused/restored SSE events while broken.
    vision_pause_announced: bool,
}

impl CaptureTrigger {
    pub(crate) fn new(
        screen_capture: Arc<ScreenCapture>,
        capture_learner: Arc<CaptureLearner>,
        decision_engine: Arc<DecisionEngine>,
        generator: Arc<ProactiveGenerator>,
        cooldown_repo: Arc<dyn CooldownRepository>,
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
            vision_failure_count: 0,
            vision_breaker_tripped_at: None,
            vision_pause_announced: false,
        }
    }

    fn vision_breaker_open(&mut self) -> bool {
        let Some(tripped_at) = self.vision_breaker_tripped_at else {
            return false;
        };
        if tripped_at.elapsed() >= VISION_FAILURE_COOLDOWN {
            // Failure count stays high until a success clears it, so the
            // breaker re-trips if vision is still broken.
            self.vision_breaker_tripped_at = None;
            info!("capture_trigger.vision_breaker_reopened");
            false
        } else {
            true
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

        if let Some(cooldown) = self.cooldown_repo.check_cooldown(
            cfg.decision.cooldown_minutes,
            cfg.decision.extended_cooldown_minutes,
        ) {
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

    async fn run_capture_cycle(&mut self) -> Option<String> {
        if self.vision_breaker_open() {
            debug!("capture_trigger.vision_breaker_open_skipping_cycle");
            return None;
        }

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
        let result = self
            .capture_learner
            .learn(
                capture_result.image,
                capture_result.active_window.as_deref(),
            )
            .await;
        self.event_queue
            .push(indicator_event(IndicatorType::Idle, None));

        match result {
            Ok(description) if !description.is_empty() => {
                self.context_count += 1;
                self.vision_failure_count = 0;
                self.vision_breaker_tripped_at = None;
                if self.vision_pause_announced {
                    self.vision_pause_announced = false;
                    self.event_queue.push(trigger_error_event(
                        "vision",
                        "Screen awareness restored.",
                        true,
                    ));
                }
                debug!(cycle = cycle_num, "capture_trigger.cycle_complete");
                Some(description)
            }
            Ok(_) => {
                // Empty description = uninformative screen, not a failure.
                self.vision_failure_count = 0;
                self.vision_breaker_tripped_at = None;
                if self.vision_pause_announced {
                    self.vision_pause_announced = false;
                    self.event_queue.push(trigger_error_event(
                        "vision",
                        "Screen awareness restored.",
                        true,
                    ));
                }
                debug!(cycle = cycle_num, "capture_trigger.empty_description");
                None
            }
            Err(e) => {
                self.vision_failure_count += 1;
                if self.vision_failure_count >= VISION_FAILURE_BREAKER_THRESHOLD {
                    self.vision_breaker_tripped_at = Some(Instant::now());
                    if !self.vision_pause_announced {
                        self.vision_pause_announced = true;
                        self.event_queue.push(trigger_error_event(
                            "vision",
                            "Screen awareness paused — vision unavailable.",
                            true,
                        ));
                    }
                    warn!(
                        cycle = cycle_num,
                        consecutive_failures = self.vision_failure_count,
                        cooldown_secs = VISION_FAILURE_COOLDOWN.as_secs(),
                        error = %e,
                        "capture_trigger.vision_breaker_tripped"
                    );
                } else {
                    warn!(
                        cycle = cycle_num,
                        consecutive_failures = self.vision_failure_count,
                        error = %e,
                        "capture_trigger.cycle_failed"
                    );
                }
                None
            }
        }
    }
}
