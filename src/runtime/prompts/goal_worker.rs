use std::path::Path;

use crate::i18n::{FALLBACK_LOCALE, t_vars};

pub struct GoalPlanningPrompt;

impl GoalPlanningPrompt {
    pub fn messages(goal_content: &str, context: &str, max_steps: u32) -> (String, String) {
        let locale = FALLBACK_LOCALE;
        let system = t_vars(
            locale,
            "prompt-goal-worker-planning-system",
            &[("max_steps", max_steps.to_string())],
        );

        let user = t_vars(
            locale,
            "prompt-goal-worker-planning-user",
            &[
                ("goal_content", goal_content.to_owned()),
                ("context", context.to_owned()),
            ],
        );

        (system, user)
    }
}

pub struct GoalExecutionPrompt;

impl GoalExecutionPrompt {
    pub fn messages(goal_content: &str, step_list: &str, work_dir: &Path) -> (String, String) {
        let locale = FALLBACK_LOCALE;
        let work_dir_str = work_dir.display();
        let system = t_vars(
            locale,
            "prompt-goal-worker-execution-system",
            &[("work_dir", work_dir_str.to_string())],
        );

        let user = t_vars(
            locale,
            "prompt-goal-worker-execution-user",
            &[
                ("goal_content", goal_content.to_owned()),
                ("step_list", step_list.to_owned()),
                ("work_dir", work_dir_str.to_string()),
            ],
        );

        (system, user)
    }
}
