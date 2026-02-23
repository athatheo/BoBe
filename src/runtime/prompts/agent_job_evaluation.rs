//! Prompt for evaluating whether a coding agent achieved its goal.

use crate::runtime::prompts::base::PromptConfig;
use crate::llm::types::AiMessage;

/// Evaluate whether a coding agent's result satisfies the original goal.
///
/// Used by `AgentJobTrigger` to decide: notify user (done) or continue agent.
pub struct AgentJobEvaluationPrompt;

impl AgentJobEvaluationPrompt {
    const SYSTEM: &str = "\
You are evaluating whether a coding agent completed its assigned task. \
The user asked the agent to do something. The agent has finished and produced a result. \
Determine if the goal was achieved based on the result summary.";

    pub fn config() -> PromptConfig {
        PromptConfig {
            temperature: 0.2,
            max_tokens: 50,
            ..PromptConfig::default()
        }
    }

    pub fn messages(
        user_intent: &str,
        result_summary: &str,
        error_message: Option<&str>,
        continuation_count: u32,
    ) -> Vec<AiMessage> {
        let mut user_parts = vec![
            format!("Original task: {user_intent}"),
            format!(
                "\nAgent result: {}",
                if result_summary.is_empty() {
                    "No summary available."
                } else {
                    result_summary
                }
            ),
        ];

        if let Some(err) = error_message {
            user_parts.push(format!("\nAgent error: {err}"));
        }

        if continuation_count > 0 {
            user_parts.push(format!(
                "\nThis agent has already been continued {continuation_count} time(s)."
            ));
        }

        user_parts.push(
            "\n\nDid the agent achieve the original task? \
             Respond with exactly one word: DONE or CONTINUE. \
             Say DONE if the task appears complete or if there were errors \
             that the agent cannot fix (e.g., missing dependencies, wrong project). \
             Say CONTINUE only if the agent made partial progress and could \
             reasonably finish with another attempt."
                .into(),
        );

        vec![
            AiMessage::system(Self::SYSTEM),
            AiMessage::user(user_parts.join("\n")),
        ]
    }
}
