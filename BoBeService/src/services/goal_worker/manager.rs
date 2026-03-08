//! GoalWorkerManager — lifecycle manager for goal workers.
//!
//! Background loop that polls for goals needing plans and approved plans
//! needing execution. Manages concurrent worker tasks with failure tracking
//! and graceful shutdown.

use std::collections::HashMap;
use std::sync::Arc;

use arc_swap::ArcSwap;
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use crate::config::Config;
use crate::db::{GoalPlanRepository, GoalRepository};
use crate::error::AppError;
use crate::models::ids::{GoalId, GoalPlanId};
use crate::models::types::{GoalPlanStatus, GoalStatus};

use super::worker::GoalWorker;

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct GoalWorkerStatus {
    pub(crate) enabled: bool,
    pub(crate) max_concurrent: u32,
    pub(crate) active_goals_count: usize,
    pub(crate) pending_approval_count: usize,
}

pub(crate) struct GoalWorkerManager {
    config: Arc<ArcSwap<Config>>,
    worker: Arc<GoalWorker>,
    goal_repo: Arc<dyn GoalRepository>,
    plan_repo: Arc<dyn GoalPlanRepository>,
    active_tasks: HashMap<GoalId, tokio::task::JoinHandle<bool>>,
    failure_counts: HashMap<GoalId, u32>,
}

impl GoalWorkerManager {
    pub(crate) fn new(
        config: Arc<ArcSwap<Config>>,
        worker: Arc<GoalWorker>,
        goal_repo: Arc<dyn GoalRepository>,
        plan_repo: Arc<dyn GoalPlanRepository>,
    ) -> Self {
        Self {
            config,
            worker,
            goal_repo,
            plan_repo,
            active_tasks: HashMap::new(),
            failure_counts: HashMap::new(),
        }
    }

    fn cfg(&self) -> arc_swap::Guard<Arc<Config>> {
        self.config.load()
    }

    pub(crate) async fn run(&mut self, mut shutdown: broadcast::Receiver<()>) {
        let cfg = self.cfg();
        if !cfg.goal_worker.enabled {
            warn!(
                enabled = false,
                hint = "Set BOBE_GOAL_WORKER_ENABLED=true",
                "goal_worker_manager.not_ready"
            );
            return;
        }

        let has_key = cfg.llm.has_anthropic_key();
        if !has_key && !cli_authenticated().await {
            warn!(
                enabled = true,
                has_api_key = false,
                hint = "Set BOBE_ANTHROPIC_API_KEY or run `claude` to authenticate",
                "goal_worker_manager.not_ready"
            );
            return;
        }

        let max_concurrent = cfg.goal_worker.max_concurrent;
        let poll_interval = cfg.goal_worker.poll_interval_seconds;
        drop(cfg);

        info!(
            max_concurrent,
            poll_interval, "goal_worker_manager.starting"
        );

        // Recover stale goals from previous run
        if let Err(e) = self.recover_stale_goals().await {
            error!(error = %e, "goal_worker_manager.recovery_failed");
        }

        loop {
            // Run one poll cycle
            if let Err(e) = self.poll_cycle().await {
                error!(error = %e, "goal_worker_manager.cycle_error");
            }

            // Interruptible sleep
            let sleep = tokio::time::sleep(std::time::Duration::from_secs(
                self.cfg().goal_worker.poll_interval_seconds,
            ));
            tokio::select! {
                () = sleep => {}
                _ = shutdown.recv() => {
                    info!("goal_worker_manager.shutdown_received");
                    break;
                }
            }
        }

        // Drain in-flight tasks
        self.drain_tasks().await;
        info!("goal_worker_manager.stopped");
    }

    async fn drain_tasks(&mut self) {
        let handles: Vec<_> = self.active_tasks.drain().map(|(_, h)| h).collect();
        for handle in handles {
            drop(handle.await);
        }
    }

    // ── Poll cycle ──────────────────────────────────────────────────────

    async fn poll_cycle(&mut self) -> Result<(), AppError> {
        self.reap_finished_tasks().await;
        self.expire_pending_plans().await?;
        self.execute_approved_plans().await?;
        self.plan_new_goals().await?;

        Ok(())
    }

    async fn reap_finished_tasks(&mut self) {
        let finished: Vec<GoalId> = self
            .active_tasks
            .iter()
            .filter(|(_, h)| h.is_finished())
            .map(|(id, _)| *id)
            .collect();

        for goal_id in finished {
            if let Some(handle) = self.active_tasks.remove(&goal_id) {
                match handle.await {
                    Ok(true) => {
                        self.failure_counts.remove(&goal_id);
                    }
                    Ok(false) => {
                        // Distinguish real failure (reset to ACTIVE) from awaiting approval
                        if let Ok(Some(goal)) = self.goal_repo.get_by_id(goal_id).await
                            && goal.status == GoalStatus::Active
                        {
                            self.record_failure(goal_id);
                            if self.is_exhausted(goal_id) {
                                self.pause_exhausted_goal(goal_id).await;
                            }
                        }
                    }
                    Err(e) => {
                        error!(
                            goal_id = %goal_id,
                            error = %e,
                            "goal_worker_manager.task_panicked"
                        );
                        self.record_failure(goal_id);
                        if self.is_exhausted(goal_id) {
                            self.pause_exhausted_goal(goal_id).await;
                        }
                    }
                }
            }
        }
    }

    // ── Recovery ────────────────────────────────────────────────────────

    async fn recover_stale_goals(&self) -> Result<(), AppError> {
        // No-op: our flow keeps goals Active until completed/paused,
        // so there are no stale "working" goals to reset on restart.
        Ok(())
    }

    // ── Expiration ──────────────────────────────────────────────────────

    async fn expire_pending_plans(&self) -> Result<(), AppError> {
        let timeout_minutes = self.cfg().goal_worker.approval_timeout_minutes as i64;
        let expired = self
            .plan_repo
            .get_expired_pending_plans(timeout_minutes)
            .await?;

        for plan in expired {
            self.plan_repo
                .update_plan_status(plan.id, GoalPlanStatus::Rejected, None)
                .await?;
            self.goal_repo
                .update_status(plan.goal_id, Some(GoalStatus::Active), None)
                .await?;
            info!(
                plan_id = %plan.id,
                goal_id = %plan.goal_id,
                "goal_worker_manager.plan_expired"
            );
        }

        Ok(())
    }

    // ── Execute approved plans ──────────────────────────────────────────

    async fn execute_approved_plans(&mut self) -> Result<(), AppError> {
        let active_goals = self.goal_repo.find_active(true).await?;

        for goal in active_goals {
            if self.is_goal_active(goal.id) {
                continue;
            }

            let active_plan = self.plan_repo.get_active_plan_for_goal(goal.id).await?;
            if let Some(plan) = active_plan
                && matches!(
                    plan.status,
                    GoalPlanStatus::Approved | GoalPlanStatus::AutoApproved
                )
            {
                info!(
                    goal_id = %goal.id,
                    plan_id = %plan.id,
                    "goal_worker_manager.executing_approved_plan"
                );
                self.spawn_execution_task(goal.id, plan.id);
            }
        }

        Ok(())
    }

    // ── Plan new goals ──────────────────────────────────────────────────

    async fn plan_new_goals(&mut self) -> Result<(), AppError> {
        let max_concurrent = self.cfg().goal_worker.max_concurrent;
        let active_count = self
            .active_tasks
            .values()
            .filter(|h| !h.is_finished())
            .count() as u32;

        if active_count >= max_concurrent {
            return Ok(());
        }

        let active_goals = self.goal_repo.find_active(true).await?;
        let slots = (max_concurrent - active_count) as usize;

        for goal in active_goals.into_iter().take(slots) {
            if self.is_goal_active(goal.id) {
                continue;
            }

            if self.is_exhausted(goal.id) {
                continue;
            }

            let existing_plan = self.plan_repo.get_active_plan_for_goal(goal.id).await?;
            if existing_plan.is_some() {
                continue;
            }

            let plans = self.plan_repo.get_plans_for_goal(goal.id).await?;
            let has_pending = plans
                .iter()
                .any(|p| p.status == GoalPlanStatus::PendingApproval);
            if has_pending {
                continue;
            }

            info!(
                goal_id = %goal.id,
                content_preview = &goal.content[..goal.content.len().min(60)],
                "goal_worker_manager.planning_goal"
            );
            self.spawn_planning_task(goal.id);
        }

        Ok(())
    }

    // ── Task spawning ───────────────────────────────────────────────────

    fn is_goal_active(&self, goal_id: GoalId) -> bool {
        self.active_tasks
            .get(&goal_id)
            .is_some_and(|h| !h.is_finished())
    }

    fn spawn_planning_task(&mut self, goal_id: GoalId) {
        let worker = Arc::clone(&self.worker);
        let goal_repo = Arc::clone(&self.goal_repo);

        let handle = tokio::spawn(async move {
            let goal = match goal_repo.get_by_id(goal_id).await {
                Ok(Some(g)) => g,
                Ok(None) => {
                    warn!(goal_id = %goal_id, "goal_worker_manager.goal_not_found");
                    return false;
                }
                Err(e) => {
                    error!(goal_id = %goal_id, error = %e, "goal_worker_manager.goal_fetch_failed");
                    return false;
                }
            };
            worker.work_on_goal(&goal).await
        });

        self.active_tasks.insert(goal_id, handle);
    }

    fn spawn_execution_task(&mut self, goal_id: GoalId, plan_id: GoalPlanId) {
        let worker = Arc::clone(&self.worker);
        let goal_repo = Arc::clone(&self.goal_repo);

        let handle = tokio::spawn(async move {
            let goal = match goal_repo.get_by_id(goal_id).await {
                Ok(Some(g)) => g,
                Ok(None) => {
                    warn!(goal_id = %goal_id, "goal_worker_manager.goal_not_found");
                    return false;
                }
                Err(e) => {
                    error!(goal_id = %goal_id, error = %e, "goal_worker_manager.goal_fetch_failed");
                    return false;
                }
            };
            match worker.execute_approved_plan(&goal, plan_id).await {
                Ok(success) => success,
                Err(e) => {
                    error!(
                        goal_id = %goal_id,
                        plan_id = %plan_id,
                        error = %e,
                        "goal_worker_manager.execution_failed"
                    );
                    false
                }
            }
        });

        self.active_tasks.insert(goal_id, handle);
    }

    // ── Failure tracking ────────────────────────────────────────────────

    fn record_failure(&mut self, goal_id: GoalId) {
        let count = self.failure_counts.entry(goal_id).or_insert(0);
        *count += 1;
    }

    fn is_exhausted(&self, goal_id: GoalId) -> bool {
        let max_retries = self.cfg().goal_worker.max_failure_retries;
        self.failure_counts
            .get(&goal_id)
            .is_some_and(|&count| count >= max_retries)
    }

    async fn pause_exhausted_goal(&mut self, goal_id: GoalId) {
        let Ok(Some(goal)) = self.goal_repo.get_by_id(goal_id).await else {
            self.failure_counts.remove(&goal_id);
            return;
        };

        let failure_count = self.failure_counts.get(&goal_id).copied().unwrap_or(0);
        let last_error = self.get_last_error(goal_id).await;

        drop(
            self.goal_repo
                .update_status(goal_id, Some(GoalStatus::Paused), None)
                .await,
        );
        self.worker
            .notify_goal_paused(&goal, failure_count, last_error.as_deref())
            .await;
        self.failure_counts.remove(&goal_id);

        warn!(
            goal_id = %goal_id,
            failure_count,
            last_error = last_error.as_deref().unwrap_or("none"),
            "goal_worker_manager.goal_paused_after_retries"
        );
    }

    async fn get_last_error(&self, goal_id: GoalId) -> Option<String> {
        let plans = self.plan_repo.get_plans_for_goal(goal_id).await.ok()?;
        for plan in &plans {
            let steps = self.plan_repo.get_steps_for_plan(plan.id).await.ok()?;
            for step in steps.iter().rev() {
                if let Some(ref err) = step.error {
                    return Some(err.clone());
                }
            }
        }
        None
    }
}

async fn cli_authenticated() -> bool {
    let Ok(claude_bin) = which::which("claude") else {
        return false;
    };

    let result = tokio::process::Command::new(claude_bin)
        .arg("auth")
        .arg("status")
        .env("CLAUDECODE", "")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await;

    match result {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            serde_json::from_str::<serde_json::Value>(&stdout)
                .ok()
                .and_then(|v| v.get("loggedIn")?.as_bool())
                .unwrap_or(false)
        }
        _ => false,
    }
}
