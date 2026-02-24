//! Prompts for the Goal Worker system (planning + execution via Claude).

use std::path::Path;

/// Prompt for generating a structured plan from a goal.
pub struct GoalPlanningPrompt;

impl GoalPlanningPrompt {
    /// Returns `(system_message, user_message)`.
    pub fn messages(goal_content: &str, context: &str, max_steps: u32) -> (String, String) {
        let system = format!(
            "You are a planning assistant. Given a goal and context, create a concrete, \
             actionable plan with numbered steps.\n\n\
             Output ONLY a JSON object with this schema:\n\
             {{\"summary\": \"Brief plan description\", \
             \"steps\": [{{\"content\": \"Step description\"}}]}}\n\n\
             Maximum {max_steps} steps. Each step should be independently \
             executable. Be specific and actionable \u{2014} not vague."
        );

        let user = format!(
            "Goal: {goal_content}\n\n\
             Context:\n{context}\n\n\
             Create an actionable plan to achieve this goal."
        );

        (system, user)
    }
}

/// Prompt for executing an approved plan in a work directory.
pub struct GoalExecutionPrompt;

impl GoalExecutionPrompt {
    /// Returns `(system_message, user_message)`.
    pub fn messages(goal_content: &str, step_list: &str, work_dir: &Path) -> (String, String) {
        let work_dir_str = work_dir.display();
        let system = format!(
            "You are an autonomous agent executing a plan for the user.\n\n\
             IMPORTANT RULES:\n\
             - Work ONLY inside this directory: {work_dir_str}\n\
             - Create all files and outputs there\n\
             - Do not open any interactive windows or editors\n\
             - Work autonomously. Do NOT ask unnecessary questions.\n\
             - If you encounter an important decision that could significantly affect \
             the outcome (e.g., choosing between fundamentally different approaches, \
             discovering the goal may be impossible, needing credentials or access), \
             use the ask_user tool.\n\
             - For minor decisions, use your best judgment and proceed.\n\
             - When done, write a brief summary to SUMMARY.md in the work directory"
        );

        let user = format!(
            "Goal: {goal_content}\n\n\
             Plan:\n{step_list}\n\n\
             Work directory: {work_dir_str}\n\n\
             Execute this plan. Create all files in the work directory. \
             When finished, write SUMMARY.md with what you did and any results."
        );

        (system, user)
    }
}
