//! Goal Worker subsystem — plans and executes goals via Claude Agent SDK.
//!
//! Architecture:
//! - `GoalExecutorProvider` trait: planning + execution backend (Claude SDK)
//! - `GoalContextProvider` trait: assembles relevant context for a goal
//! - `GoalWorker`: single-goal lifecycle orchestrator
//! - `GoalWorkerManager`: background loop managing concurrent goal workers

pub mod claude_provider;
pub mod context_provider;
pub mod manager;
pub mod worker;

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::goal::Goal;
use crate::models::goal_plan::{GoalPlan, GoalPlanStep};

// ─── Shared types ───────────────────────────────────────────────────────────

/// A step produced by the planning phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub content: String,
    pub order: i32,
}

/// Result of executing an entire goal plan.
#[derive(Debug, Clone)]
pub struct GoalExecutionResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
}

// ─── Traits ─────────────────────────────────────────────────────────────────

/// Backend for goal planning and execution (e.g. Claude Agent SDK).
#[async_trait]
pub trait GoalExecutorProvider: Send + Sync {
    /// Create a dedicated work directory for a goal.
    fn create_work_dir(&self, goal_id: Uuid, goal_title: &str) -> PathBuf;

    /// Generate a plan for achieving a goal.
    async fn generate_plan(
        &self,
        goal: &Goal,
        context: &str,
        max_steps: Option<u32>,
    ) -> Result<Vec<PlanStep>, AppError>;

    /// Execute an approved plan in a work directory.
    async fn execute_goal(
        &self,
        goal: &Goal,
        plan: &GoalPlan,
        steps: &[GoalPlanStep],
        work_dir: &Path,
    ) -> Result<GoalExecutionResult, AppError>;
}

/// Assembles context relevant to a goal (memories, other goals, soul docs).
#[async_trait]
pub trait GoalContextProvider: Send + Sync {
    async fn get_context_for_goal(&self, goal: &Goal) -> Result<String, AppError>;
}
