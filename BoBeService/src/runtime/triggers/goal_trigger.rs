//! Decide worker reads goals dir via its skill; we pass only the title as a hint.

use std::sync::Arc;

use arc_swap::ArcSwap;
use tracing::{debug, info, warn};

use crate::config::Config;
use crate::db::SqliteCooldownRepo;
use crate::runtime::proactive_generator::ProactiveGenerator;
use crate::runtime::state::Decision;
use crate::runtime::turn_admission::{TurnAdmission, TurnSource};
use crate::services::goals::goals_service::GoalsService;
use crate::util::sse::event_queue::EventQueue;
use crate::util::sse::indicator_guard::IndicatorGuard;
use crate::util::sse::types::IndicatorType;

pub(crate) struct GoalTrigger {
    goals_service: Arc<GoalsService>,
    generator: Arc<ProactiveGenerator>,
    cooldown_repo: Arc<SqliteCooldownRepo>,
    event_queue: Arc<EventQueue>,
    config: Arc<ArcSwap<Config>>,
    turn_admission: Arc<TurnAdmission>,
}

impl GoalTrigger {
    pub(crate) fn new(
        goals_service: Arc<GoalsService>,
        generator: Arc<ProactiveGenerator>,
        cooldown_repo: Arc<SqliteCooldownRepo>,
        event_queue: Arc<EventQueue>,
        config: Arc<ArcSwap<Config>>,
        turn_admission: Arc<TurnAdmission>,
    ) -> Self {
        Self {
            goals_service,
            generator,
            cooldown_repo,
            event_queue,
            config,
            turn_admission,
        }
    }

    pub(crate) async fn fire(&self) -> Decision {
        let Some(_turn) = self.turn_admission.try_admit(TurnSource::Goal) else {
            debug!("goal_trigger.turn_busy");
            return Decision::Idle;
        };
        let cfg = self.config.load();

        if let Some(cooldown) = self.cooldown_repo.check_cooldown(
            cfg.decision.cooldown_minutes,
            cfg.decision.extended_cooldown_minutes,
        ) {
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

        // RAII: one Thinking → Idle bracket for the whole goal scan. Without
        // hoisting, N goals produced 2N indicator events. EventQueue's
        // set_indicator short-circuits no-ops so iteration is silent.
        let indicator_guard = IndicatorGuard::new(Arc::clone(&self.event_queue));
        self.event_queue.set_indicator(IndicatorType::Thinking);

        let goal_summary = goals
            .iter()
            .take(12)
            .map(|goal| format!("- {}", goal.title))
            .collect::<Vec<_>>()
            .join("\n");
        drop(indicator_guard);
        let decision = self
            .generator
            .generate_proactive_response(
                cfg.conversation.auto_close_minutes as i64,
                Some(format!("Active user goals:\n{goal_summary}")),
            )
            .await;
        if decision == Decision::Engage {
            info!(
                goal_count = goals.len(),
                "goal_trigger.engagement_triggered"
            );
        } else {
            debug!("goal_trigger.no_engagement");
        }
        decision
    }
}
