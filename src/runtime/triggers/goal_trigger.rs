//! Goal trigger — checks if active goals are relevant to current context.
//!
//! Fetches active goals, asks DecisionEngine per-goal, stops after first engagement.

use std::sync::Arc;

use tracing::{debug, info, warn};

use crate::util::sse::event_queue::EventQueue;
use crate::util::sse::types::IndicatorType;
use crate::runtime::decision_engine::DecisionEngine;
use crate::runtime::proactive_generator::ProactiveGenerator;
use crate::runtime::state::{
    Decision, OrchestratorConfig, TriggerContext, TriggerType,
};
use crate::db::CooldownRepository;
use crate::db::GoalRepository;

pub struct GoalTrigger {
    goal_repo: Arc<dyn GoalRepository>,
    decision_engine: Arc<DecisionEngine>,
    generator: Arc<ProactiveGenerator>,
    cooldown_repo: Option<Arc<dyn CooldownRepository>>,
    event_queue: Arc<EventQueue>,
    config: OrchestratorConfig,
}

impl GoalTrigger {
    pub fn new(
        goal_repo: Arc<dyn GoalRepository>,
        decision_engine: Arc<DecisionEngine>,
        generator: Arc<ProactiveGenerator>,
        cooldown_repo: Option<Arc<dyn CooldownRepository>>,
        event_queue: Arc<EventQueue>,
        config: OrchestratorConfig,
    ) -> Self {
        Self {
            goal_repo,
            decision_engine,
            generator,
            cooldown_repo,
            event_queue,
            config,
        }
    }

    pub fn update_config(&mut self, config: OrchestratorConfig) {
        self.config = config;
    }

    /// Execute the goal trigger. Returns `Decision::Engage` if engagement was triggered.
    pub async fn fire(&self) -> Decision {
        // Cooldown check
        if let Some(ref cooldown_repo) = self.cooldown_repo
            && let Some(cooldown) = cooldown_repo.check_cooldown(
                self.config.decision_cooldown_minutes,
                self.config.decision_extended_cooldown_minutes,
            )
        {
            debug!(
                remaining_s = cooldown.remaining.num_seconds(),
                "goal_trigger.cooldown_active"
            );
            return Decision::Idle;
        }

        let goals = match self.goal_repo.find_active(true).await {
            Ok(g) => g,
            Err(e) => {
                warn!(error = %e, "goal_trigger.fetch_failed");
                return Decision::Idle;
            }
        };

        if goals.is_empty() {
            debug!("goal_trigger.no_active_goals");
            return Decision::Idle;
        }

        info!(goal_count = goals.len(), "goal_trigger.checking_goals");

        for goal in &goals {
            self.event_queue.set_indicator(IndicatorType::Thinking);

            let context = TriggerContext {
                trigger_type: TriggerType::Goal,
                context_text: goal.content.clone(),
                observation: None,
                goal: Some(goal.clone()),
            };

            let decision = self.decision_engine.decide(&context).await;

            if decision == Decision::Engage {
                info!(
                    goal_id = %goal.id,
                    goal_content = &goal.content[..goal.content.len().min(50)],
                    "goal_trigger.engagement_triggered"
                );
                self.generator
                    .generate_proactive_response(
                        self.config.conversation_auto_close_minutes as i64,
                        Some(format!("User's goal: {}", goal.content)),
                    )
                    .await;
                return Decision::Engage;
            }

            self.event_queue.set_indicator(IndicatorType::Idle);
        }

        debug!("goal_trigger.no_engagement");
        Decision::Idle
    }
}
