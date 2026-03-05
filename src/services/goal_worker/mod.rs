//! Goal Worker subsystem — plans and executes goals via Claude Agent SDK.
//!
//! Architecture:
//! - `GoalExecutorProvider` trait: planning + execution backend (Claude SDK)
//! - `GoalContextProvider` trait: assembles relevant context for a goal
//! - `GoalWorker`: single-goal lifecycle orchestrator
//! - `GoalWorkerManager`: background loop managing concurrent goal workers

pub(crate) mod claude_provider;
pub(crate) mod context_provider;
pub(crate) mod manager;
pub(crate) mod worker;

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::goal::Goal;
use crate::models::goal_plan::{GoalPlan, GoalPlanStep};

// ─── Shared types ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PlanStep {
    pub(crate) content: String,
    pub(crate) order: i32,
}

#[derive(Debug, Clone)]
pub(crate) struct GoalExecutionResult {
    pub(crate) success: bool,
    pub(crate) output: String,
    pub(crate) error: Option<String>,
}

// ─── Traits ─────────────────────────────────────────────────────────────────

#[async_trait]
pub(crate) trait GoalExecutorProvider: Send + Sync {
    fn create_work_dir(&self, goal_id: Uuid, goal_title: &str) -> PathBuf;

    async fn generate_plan(
        &self,
        goal: &Goal,
        context: &str,
        max_steps: Option<u32>,
    ) -> Result<Vec<PlanStep>, AppError>;

    async fn execute_goal(
        &self,
        goal: &Goal,
        plan: &GoalPlan,
        steps: &[GoalPlanStep],
        work_dir: &Path,
    ) -> Result<GoalExecutionResult, AppError>;
}

#[async_trait]
pub(crate) trait GoalContextProvider: Send + Sync {
    async fn get_context_for_goal(&self, goal: &Goal) -> Result<String, AppError>;
}
