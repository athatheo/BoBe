//! Capture trigger — entry point for capture-based proactive engagement.
//!
//! Orchestrates: screenshot → learn → cooldown check → decision → response.

use std::sync::Arc;

use tracing::{debug, error, info};

use crate::application::learners::types::LearnerObservation;
use crate::application::learners::CaptureLearner;
use crate::application::runtime::state::{Decision, OrchestratorConfig, TriggerContext, TriggerType};
use crate::domain::observation::Observation;
use crate::ports::repos::cooldown_repo::CooldownRepository;

use super::super::runtime::decision_engine::DecisionEngine;
use super::super::runtime::proactive_generator::ProactiveGenerator;

pub struct CaptureTrigger {
    capture_learner: Arc<CaptureLearner>,
    decision_engine: Arc<DecisionEngine>,
    generator: Arc<ProactiveGenerator>,
    cooldown_repo: Option<Arc<dyn CooldownRepository>>,
    config: OrchestratorConfig,
    enabled: bool,
    context_count: usize,
}

impl CaptureTrigger {
    pub fn new(
        capture_learner: Arc<CaptureLearner>,
        decision_engine: Arc<DecisionEngine>,
        generator: Arc<ProactiveGenerator>,
        cooldown_repo: Option<Arc<dyn CooldownRepository>>,
        config: OrchestratorConfig,
    ) -> Self {
        Self {
            capture_learner,
            decision_engine,
            generator,
            cooldown_repo,
            config,
            enabled: false,
            context_count: 0,
        }
    }

    pub fn update_config(&mut self, config: OrchestratorConfig) {
        self.config = config;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn context_count(&self) -> usize {
        self.context_count
    }

    pub async fn start(&mut self) {
        self.enabled = true;
        info!("capture_trigger.started");
    }

    pub async fn stop(&mut self) {
        self.enabled = false;
        info!("capture_trigger.stopped");
    }

    /// Execute the capture trigger with pre-captured screenshot data.
    pub async fn fire(&mut self, screenshot: Vec<u8>, active_window: Option<String>) -> Decision {
        let observation = self.run_capture_cycle(screenshot, active_window).await;
        let Some(obs) = observation else {
            return Decision::Idle;
        };

        // Cooldown check
        if let Some(ref cooldown_repo) = self.cooldown_repo {
            if let Some(cooldown) = cooldown_repo.check_cooldown(
                self.config.decision_cooldown_minutes,
                self.config.decision_extended_cooldown_minutes,
            ) {
                debug!(
                    remaining_s = cooldown.remaining.num_seconds(),
                    cooldown_type = %cooldown.cooldown_type,
                    "capture_trigger.cooldown_active"
                );
                return Decision::Idle;
            }
        }

        // Decision
        let context = TriggerContext {
            trigger_type: TriggerType::Capture,
            context_text: obs.content.clone(),
        };

        let decision = self.decision_engine.decide(&context).await;

        if decision == Decision::Engage {
            self.generator.generate_proactive_response(
                self.config.conversation_auto_close_minutes as i64,
                None,
            ).await;
        }

        decision
    }

    async fn run_capture_cycle(
        &mut self,
        screenshot: Vec<u8>,
        active_window: Option<String>,
    ) -> Option<Observation> {
        let cycle_num = self.context_count + 1;

        info!(cycle = cycle_num, "capture_trigger.cycle_start");

        let observation = LearnerObservation::capture(screenshot, active_window);
        match self.capture_learner.learn(&observation).await {
            Ok(result) => {
                self.context_count += 1;
                debug!(cycle = cycle_num, "capture_trigger.cycle_complete");
                // Return a minimal Observation for the decision engine context
                Some(Observation::new(
                    crate::domain::types::ObservationSource::Screen,
                    observation.text.unwrap_or_default(),
                    "screen".into(),
                ))
            }
            Err(e) => {
                error!(error = %e, cycle = cycle_num, "capture_trigger.cycle_failed");
                None
            }
        }
    }
}
