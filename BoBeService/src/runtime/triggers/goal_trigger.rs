//! Checks active goals against current context, engages on first match.
//!
//! Goals are file-backed (`~/.bobe/goals/<id>.md`); we read them
//! through `GoalsService` rather than `GoalRepository` (which is
//! gone). The Decide worker has access to the goals dir directly via
//! its skill, so we only need to pass the goal title as
//! `context_text` to give the engine a starting hint.

use std::sync::Arc;

use arc_swap::ArcSwap;
use tracing::{debug, info, warn};

use crate::config::Config;
use crate::db::CooldownRepository;
use crate::runtime::decision_engine::DecisionEngine;
use crate::runtime::proactive_generator::ProactiveGenerator;
use crate::runtime::state::{Decision, TriggerContext, TriggerType};
use crate::services::goals::goals_service::GoalsService;
use crate::util::sse::event_queue::EventQueue;
use crate::util::sse::types::IndicatorType;

pub(crate) struct GoalTrigger {
    goals_service: Arc<GoalsService>,
    decision_engine: Arc<DecisionEngine>,
    generator: Arc<ProactiveGenerator>,
    cooldown_repo: Option<Arc<dyn CooldownRepository>>,
    event_queue: Arc<EventQueue>,
    config: Arc<ArcSwap<Config>>,
}

impl GoalTrigger {
    pub(crate) fn new(
        goals_service: Arc<GoalsService>,
        decision_engine: Arc<DecisionEngine>,
        generator: Arc<ProactiveGenerator>,
        cooldown_repo: Option<Arc<dyn CooldownRepository>>,
        event_queue: Arc<EventQueue>,
        config: Arc<ArcSwap<Config>>,
    ) -> Self {
        Self {
            goals_service,
            decision_engine,
            generator,
            cooldown_repo,
            event_queue,
            config,
        }
    }

    pub(crate) async fn fire(&self) -> Decision {
        let cfg = self.config.load();

        if let Some(ref cooldown_repo) = self.cooldown_repo
            && let Some(cooldown) = cooldown_repo.check_cooldown(
                cfg.decision.cooldown_minutes,
                cfg.decision.extended_cooldown_minutes,
            )
        {
            debug!(
                remaining_s = cooldown.remaining.num_seconds(),
                "goal_trigger.cooldown_active"
            );
            return Decision::Idle;
        }

        let goals = match self.goals_service.list_active().await {
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
                context_text: goal.title.clone(),
            };

            let decision = self.decision_engine.decide(&context).await;

            if decision == Decision::Engage {
                info!(
                    goal_id = %goal.id,
                    title = &goal.title[..goal.title.len().min(50)],
                    "goal_trigger.engagement_triggered"
                );
                self.generator
                    .generate_proactive_response(
                        cfg.conversation.auto_close_minutes as i64,
                        Some(format!("User's goal: {}", goal.title)),
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
