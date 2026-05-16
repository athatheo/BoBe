//! Separate from chat agent so "do not engage" produces no user-visible reply.

use std::sync::Arc;

use arc_swap::ArcSwap;
use chrono::{Duration, Local, Utc};
use serde_json::{Value, json};
use tracing::{debug, warn};
use uuid::Uuid;

use crate::config::Config;
use crate::copilot::registry::WorkerRegistry;
use crate::copilot::types::JobInput;
use crate::runtime::conversation_service::ConversationService;
use crate::runtime::state::{Decision, TriggerContext, TriggerType};
use crate::util::text::truncate_str;

pub(crate) struct DecisionEngine {
    workers: Arc<WorkerRegistry>,
    conversation: Arc<ConversationService>,
    config: Arc<ArcSwap<Config>>,
}

impl DecisionEngine {
    pub(crate) fn new(
        workers: Arc<WorkerRegistry>,
        conversation: Arc<ConversationService>,
        config: Arc<ArcSwap<Config>>,
    ) -> Self {
        Self {
            workers,
            conversation,
            config,
        }
    }

    pub(crate) async fn decide(&self, context: &TriggerContext) -> Decision {
        match context.trigger_type {
            TriggerType::Capture => self.decide_on_capture(&context.context_text).await,
            TriggerType::Goal => self.decide_on_goal(&context.context_text).await,
            TriggerType::Checkin => Decision::Engage,
        }
    }

    async fn decide_on_capture(&self, current_text: &str) -> Decision {
        if self.blocked_by_active_conversation().await {
            return Decision::Idle;
        }

        let cfg = self.config.load();
        let recent_ai_messages = self
            .conversation
            .get_recent_ai_messages(cfg.decision.recent_ai_messages_limit)
            .await
            .unwrap_or_default();

        let input = json!({
            "trigger_kind": "capture",
            "current_activity": truncate_str(current_text, 600),
            "recent_ai_messages": recent_ai_messages
                .iter()
                .map(|m| truncate_str(m, 200).to_string())
                .collect::<Vec<_>>(),
            "current_time": Local::now().format("%Y-%m-%d %H:%M:%S %z").to_string(),
        });
        self.run_decision("capture_engagement_decision", input)
            .await
    }

    async fn decide_on_goal(&self, goal_content: &str) -> Decision {
        if self.blocked_by_active_conversation().await {
            return Decision::Idle;
        }

        let input = json!({
            "trigger_kind": "goal",
            "current_activity": truncate_str(goal_content, 600),
            "recent_ai_messages": Vec::<String>::new(),
            "current_time": Local::now().format("%Y-%m-%d %H:%M:%S %z").to_string(),
        });
        self.run_decision("goal_engagement_decision", input).await
    }

    /// Suppress proactive engagement while a conversation is still warm.
    async fn blocked_by_active_conversation(&self) -> bool {
        let Ok(Some(active)) = self.conversation.get_pending_or_active().await else {
            return false;
        };
        let cfg = self.config.load();
        let timeout = Duration::seconds(cfg.conversation.inactivity_timeout_seconds as i64);
        let time_since = Utc::now() - active.updated_at;
        if time_since < timeout {
            debug!(
                conversation_id = %active.id,
                "decision_engine.blocked_by_recent_conversation"
            );
            true
        } else {
            debug!(
                conversation_id = %active.id,
                stale_seconds = time_since.num_seconds(),
                "decision_engine.conversation_stale_allowing_reachout"
            );
            false
        }
    }

    /// Any failure path falls back to `Idle` — when in doubt, leave user alone.
    async fn run_decision(&self, kind: &str, input: Value) -> Decision {
        let worker = match self.workers.decide().await {
            Ok(w) => w,
            Err(e) => {
                warn!(error = %e, "decision_engine.decide_worker_unavailable");
                return Decision::Idle;
            }
        };

        let job = JobInput {
            job_id: Uuid::new_v4(),
            kind: kind.into(),
            instructions: "Decide whether BoBe should engage the user proactively right now. \
                 Output strict JSON: output must be \
                 {\"decision\":\"reach_out|idle|need_more_info\",\"reasoning\":\"<short>\"}. \
                 Default to idle when uncertain. See your SKILL.md for heuristics."
                .into(),
            input,
        };

        match worker.submit(job).await {
            Ok(out) => parse_decision(&out.output),
            Err(e) => {
                warn!(error = %e, "decision_engine.decide_submit_failed");
                Decision::Idle
            }
        }
    }
}

fn parse_decision(output: &Value) -> Decision {
    let decision = output
        .get("decision")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let reasoning = output
        .get("reasoning")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    debug!(
        decision = %decision,
        reasoning = truncate_str(reasoning, 150),
        "decision_engine.parsed"
    );
    match decision {
        "reach_out" => Decision::Engage,
        "need_more_info" => Decision::NeedMoreInfo,
        _ => Decision::Idle,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_decision_reach_out() {
        let v = json!({"decision": "reach_out", "reasoning": "user stuck"});
        assert_eq!(parse_decision(&v), Decision::Engage);
    }

    #[test]
    fn parse_decision_need_more_info() {
        let v = json!({"decision": "need_more_info", "reasoning": "ambiguous"});
        assert_eq!(parse_decision(&v), Decision::NeedMoreInfo);
    }

    #[test]
    fn parse_decision_idle_default() {
        let v = json!({"decision": "shrug", "reasoning": ""});
        assert_eq!(parse_decision(&v), Decision::Idle);
    }

    #[test]
    fn parse_decision_missing_fields() {
        let v = json!({});
        assert_eq!(parse_decision(&v), Decision::Idle);
    }

    #[test]
    fn parse_decision_handles_multibyte_reasoning() {
        // Regression: byte-slicing `len().min(150)` could land mid-codepoint.
        let mostly_ascii = "x".repeat(140);
        let trailing_emoji = "\u{1F600}\u{1F600}\u{1F600}\u{1F600}";
        let reasoning = format!("{mostly_ascii}{trailing_emoji}");
        let v = json!({"decision": "reach_out", "reasoning": reasoning});
        assert_eq!(parse_decision(&v), Decision::Engage);
    }
}
