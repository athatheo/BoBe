//! GoalWorker — orchestrates goal work: plan → approve → execute → check in.
//!
//! Lifecycle: gather context → plan → await approval → execute silently → notify.
//! Claude works in a dedicated folder (`~/.bobe/goal-work/<slug>/`).
//! No intermediate notifications — just one check-in when done.

use std::sync::Arc;

use arc_swap::ArcSwap;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::config::Config;
use crate::db::{GoalPlanRepository, GoalRepository};
use crate::error::AppError;
use crate::models::goal::Goal;
use crate::models::types::{GoalPlanStatus, GoalPlanStepStatus, GoalStatus, TurnRole};
use crate::runtime::response_streamer::stream_simple_message;
use crate::services::conversation_service::ConversationService;
use crate::util::sse::event_queue::EventQueue;

use super::{GoalContextProvider, GoalExecutorProvider, PlanStep};

pub(crate) struct GoalWorker {
    config: Arc<ArcSwap<Config>>,
    executor: Arc<dyn GoalExecutorProvider>,
    context_provider: Arc<dyn GoalContextProvider>,
    goal_repo: Arc<dyn GoalRepository>,
    plan_repo: Arc<dyn GoalPlanRepository>,
    event_queue: Arc<EventQueue>,
    conversation_service: Arc<ConversationService>,
}

impl GoalWorker {
    pub(crate) fn new(
        config: Arc<ArcSwap<Config>>,
        executor: Arc<dyn GoalExecutorProvider>,
        context_provider: Arc<dyn GoalContextProvider>,
        goal_repo: Arc<dyn GoalRepository>,
        plan_repo: Arc<dyn GoalPlanRepository>,
        event_queue: Arc<EventQueue>,
        conversation_service: Arc<ConversationService>,
    ) -> Self {
        Self {
            config,
            executor,
            context_provider,
            goal_repo,
            plan_repo,
            event_queue,
            conversation_service,
        }
    }

    fn cfg(&self) -> arc_swap::Guard<Arc<Config>> {
        self.config.load()
    }

    pub(crate) async fn work_on_goal(&self, goal: &Goal) -> bool {
        let goal_id = goal.id;
        let autonomous = self.cfg().goal_worker.autonomous;

        info!(
            goal_id = %goal_id,
            content_preview = &goal.content[..goal.content.len().min(60)],
            autonomous,
            "goal_worker.starting"
        );

        match self.work_on_goal_inner(goal, autonomous).await {
            Ok(completed) => completed,
            Err(e) => {
                error!(
                    goal_id = %goal_id,
                    error = %e,
                    "goal_worker.error"
                );
                // Reset goal to active so it can be retried
                let _ = self
                    .goal_repo
                    .update_status(goal_id, Some(GoalStatus::Active), None)
                    .await;
                false
            }
        }
    }

    async fn work_on_goal_inner(&self, goal: &Goal, autonomous: bool) -> Result<bool, AppError> {
        let goal_id = goal.id;

        let context = self.context_provider.get_context_for_goal(goal).await?;

        let plan_steps = self.executor.generate_plan(goal, &context, None).await?;
        if plan_steps.is_empty() {
            warn!(goal_id = %goal_id, "goal_worker.empty_plan");
            return Ok(false);
        }

        let summary = format!("Plan for: {}", &goal.content[..goal.content.len().min(100)]);
        let initial_status = if autonomous {
            GoalPlanStatus::AutoApproved
        } else {
            GoalPlanStatus::PendingApproval
        };
        let plan = self
            .plan_repo
            .create_plan(goal_id, &summary, initial_status)
            .await?;

        for step in &plan_steps {
            self.plan_repo
                .create_step(plan.id, step.order, &step.content)
                .await?;
        }

        if autonomous {
            self.notify_started(goal).await;

            info!(
                goal_id = %goal_id,
                plan_id = %plan.id,
                step_count = plan_steps.len(),
                "goal_worker.auto_executing"
            );
            return self.execute_approved_plan(goal, plan.id).await;
        }

        self.notify_plan_ready(goal, &plan_steps).await;

        info!(
            goal_id = %goal_id,
            plan_id = %plan.id,
            step_count = plan_steps.len(),
            "goal_worker.plan_created"
        );
        Ok(false) // Not complete yet — waiting for approval
    }

    pub(crate) async fn execute_approved_plan(
        &self,
        goal: &Goal,
        plan_id: Uuid,
    ) -> Result<bool, AppError> {
        let plan = self.plan_repo.get_plan(plan_id).await?;
        let Some(plan) = plan else {
            error!(plan_id = %plan_id, "goal_worker.plan_not_found");
            return Ok(false);
        };

        let goal_id = goal.id;
        let steps = self.plan_repo.get_steps_for_plan(plan_id).await?;

        info!(
            goal_id = %goal_id,
            plan_id = %plan_id,
            step_count = steps.len(),
            "goal_worker.executing_plan"
        );

        self.plan_repo
            .update_plan_status(plan_id, GoalPlanStatus::InProgress, None)
            .await?;

        let work_dir = self.executor.create_work_dir(goal_id, &goal.content);

        let pending_steps: Vec<_> = steps
            .iter()
            .filter(|s| {
                !matches!(
                    s.status,
                    GoalPlanStepStatus::Completed | GoalPlanStepStatus::Skipped
                )
            })
            .cloned()
            .collect();

        let result = self
            .executor
            .execute_goal(goal, &plan, &pending_steps, &work_dir)
            .await?;

        let final_status = if result.success {
            GoalPlanStepStatus::Completed
        } else {
            GoalPlanStepStatus::Failed
        };

        for step in &pending_steps {
            let step_result = if result.success {
                Some(result.output.get(..500).unwrap_or(&result.output))
            } else {
                None
            };
            let step_error = if result.success {
                None
            } else {
                result.error.as_deref()
            };
            let _ = self
                .plan_repo
                .update_step_status(step.id, final_status, step_result, step_error)
                .await;
        }

        if result.success {
            self.plan_repo
                .update_plan_status(plan_id, GoalPlanStatus::Completed, None)
                .await?;
            self.goal_repo
                .update_status(goal_id, Some(GoalStatus::Completed), None)
                .await?;
            self.notify_goal_complete(goal, &work_dir).await;
        } else {
            self.plan_repo
                .update_plan_status(plan_id, GoalPlanStatus::Failed, result.error.as_deref())
                .await?;
            self.goal_repo
                .update_status(goal_id, Some(GoalStatus::Active), None)
                .await?;
        }

        Ok(result.success)
    }

    // ── Notifications ───────────────────────────────────────────────────

    async fn notify_started(&self, goal: &Goal) {
        let message = format!("Working on: **{}**", goal.content);
        stream_simple_message(&message, &self.event_queue, None);
        self.persist_as_turn(&message).await;
    }

    async fn notify_plan_ready(&self, goal: &Goal, steps: &[PlanStep]) {
        let step_list: String = steps
            .iter()
            .enumerate()
            .map(|(i, s)| format!("  {}. {}", i + 1, s.content))
            .collect::<Vec<_>>()
            .join("\n");

        let message = format!(
            "I've created a plan for your goal: **{}**\n\n\
             Steps:\n{}\n\n\
             Would you like me to go ahead with this plan? \
             You can approve it by saying \"yes\" or \"approve the plan\".",
            goal.content, step_list
        );
        stream_simple_message(&message, &self.event_queue, None);
        self.persist_as_turn(&message).await;
    }

    async fn notify_goal_complete(&self, goal: &Goal, work_dir: &std::path::Path) {
        let message = format!(
            "Your goal is ready: **{}**\n\n\
             Everything is in `{}`\n\n\
             Take a look and let me know if you'd like any changes.",
            goal.content,
            work_dir.display()
        );
        stream_simple_message(&message, &self.event_queue, None);
        self.persist_as_turn(&message).await;
    }

    pub(crate) async fn notify_goal_paused(
        &self,
        goal: &Goal,
        failure_count: u32,
        last_error: Option<&str>,
    ) {
        let error_detail = last_error
            .map(|e| format!("\n\nLast error: {e}"))
            .unwrap_or_default();
        let message = format!(
            "I've paused your goal: **{}**\n\n\
             It failed {failure_count} times in a row.{error_detail}\n\n\
             You can resume it when ready, or update the goal if needed.",
            goal.content
        );
        stream_simple_message(&message, &self.event_queue, None);
        self.persist_as_turn(&message).await;
    }

    async fn persist_as_turn(&self, message: &str) {
        match self.conversation_service.get_pending_or_active().await {
            Ok(Some(conv)) => {
                if let Err(e) = self
                    .conversation_service
                    .add_turn(conv.id, TurnRole::Assistant, message)
                    .await
                {
                    warn!(error = %e, "goal_worker.persist_turn_failed");
                }
            }
            Ok(None) => {
                if let Err(e) = self.conversation_service.create_pending(message).await {
                    warn!(error = %e, "goal_worker.create_pending_failed");
                }
            }
            Err(e) => {
                warn!(error = %e, "goal_worker.persist_turn_failed");
            }
        }
    }
}
