use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
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

/// RAII release of the shared in-flight flag. Mirrors `UserMessageGuard`
/// in `runtime/session.rs` but lives here so the capture trigger doesn't
/// import RuntimeSession (circular: RuntimeSession owns CaptureTrigger).
struct InFlightGuard {
    flag: Arc<AtomicBool>,
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::Release);
    }
}

pub(crate) struct CaptureTrigger {
    screen_capture: Arc<ScreenCapture>,
    capture_learner: Arc<CaptureLearner>,
    decision_engine: Arc<DecisionEngine>,
    generator: Arc<ProactiveGenerator>,
    cooldown_repo: Arc<dyn CooldownRepository>,
    event_queue: Arc<EventQueue>,
    config: Arc<ArcSwap<Config>>,
    /// Same Arc as `RuntimeSession.user_message_in_flight`. CAS true at
    /// the top of `fire()` to serialize with text + voice turns; skip the
    /// cycle if another turn is already running. Without this gate the
    /// capture trigger's indicator pushes (`ScreenCapture` → `Thinking`
    /// → `Idle`) overwrote whatever indicator a concurrent voice or
    /// text turn had set, leaving the Swift store out of sync.
    user_message_in_flight: Arc<AtomicBool>,
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
        user_message_in_flight: Arc<AtomicBool>,
    ) -> Self {
        Self {
            screen_capture,
            capture_learner,
            decision_engine,
            generator,
            cooldown_repo,
            event_queue,
            config,
            user_message_in_flight,
            enabled: false,
            context_count: 0,
            vision_failure_count: 0,
            vision_breaker_tripped_at: None,
            vision_pause_announced: false,
        }
    }

    /// Reset vision-breaker state after a successful capture and surface
    /// the SSE recovery notice if we were previously paused. Idempotent.
    fn announce_vision_recovered(&mut self) {
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
    }

    /// CAS the shared in-flight flag; returns a guard that releases on drop.
    /// `None` means another turn is already running — caller should skip.
    fn try_acquire_in_flight(&self) -> Option<InFlightGuard> {
        if self
            .user_message_in_flight
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return None;
        }
        Some(InFlightGuard {
            flag: Arc::clone(&self.user_message_in_flight),
        })
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
        // Single-flight gate: skip if a text- or voice-turn is in flight
        // so concurrent indicators don't race. RAII guard releases on
        // every return path including panic-unwind.
        let Some(_in_flight_guard) = self.try_acquire_in_flight() else {
            debug!("capture_trigger.skipped_user_message_in_flight");
            return Decision::Idle;
        };

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
                self.announce_vision_recovered();
                debug!(cycle = cycle_num, "capture_trigger.cycle_complete");
                Some(description)
            }
            Ok(_) => {
                // Empty description = uninformative screen, not a failure.
                self.announce_vision_recovered();
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
