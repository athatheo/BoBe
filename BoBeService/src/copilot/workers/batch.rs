//! `BatchWorker` — `AgentWorker` impl for headless batch jobs (goals,
//! observe, vision, consolidate). Sessions run in `autopilot` mode so
//! the model agent-loops to completion (auto-nudged toward
//! `task_complete`); we block on `send_and_wait`, parse the assistant
//! message as JSON, and surface the structured `output` field.
//!
//! Submission is serialized per worker — only one job in flight at a
//! time. The SDK's `idle_waiter` slot is also a single-flight gate, but
//! holding our own mutex preserves strict job ordering and means
//! cancellation in `submit()` doesn't race a queued caller.

#![allow(
    dead_code,
    reason = "Phase 6: type + accessors complete; Phase 5 wires consumers"
)]

use std::sync::Arc;

use async_trait::async_trait;
use github_copilot_sdk::session::Session;
use github_copilot_sdk::types::MessageOptions;
use tokio::sync::Mutex;

use crate::copilot::error::WorkerError;
use crate::copilot::types::{JobInput, JobOutput, WorkerClass};

use super::AgentWorker;

pub(crate) struct BatchWorker {
    class: WorkerClass,
    session: Arc<Session>,
    submit_lock: Mutex<()>,
}

impl BatchWorker {
    pub(crate) fn new(class: WorkerClass, session: Arc<Session>) -> Arc<Self> {
        Arc::new(Self {
            class,
            session,
            submit_lock: Mutex::new(()),
        })
    }

    pub(crate) fn class(&self) -> WorkerClass {
        self.class
    }

    /// Inherent equivalent of [`AgentWorker::submit`] so callers
    /// holding an `Arc<BatchWorker>` (the typed accessor return type)
    /// don't need the trait in scope. The trait impl just forwards.
    pub(crate) async fn submit(&self, job: JobInput) -> Result<JobOutput, WorkerError> {
        let _guard = self.submit_lock.lock().await;

        let prompt = format!(
            "Job {job_id} ({kind}). {instructions}\n\n\
             Respond with a single JSON object only — no preamble, no code fences. \
             Required shape: {{\"job_id\":\"{job_id}\",\"output\":<...>}}.\n\n\
             input = {input}",
            job_id = job.job_id,
            kind = job.kind,
            instructions = job.instructions,
            input = serde_json::to_string(&job.input).unwrap_or_else(|_| "null".into()),
        );

        // Session mode (autopilot vs interactive) is set on `SessionConfig`
        // at create time — see `registry::create_or_resume`. `MessageOptions::with_mode`
        // controls *delivery* (Enqueue vs Immediate) which we leave defaulted.
        let opts = MessageOptions::new(prompt).with_wait_timeout(self.class.turn_timeout());

        let event = self
            .session
            .send_and_wait(opts)
            .await?
            .ok_or(WorkerError::NoAssistantMessage(job.job_id))?;

        let text = event
            .data
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        let parsed = extract_json(&text).ok_or(WorkerError::NoJsonOutput(job.job_id))?;

        Ok(JobOutput {
            job_id: job.job_id,
            output: parsed
                .get("output")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
            text,
            error: parsed
                .get("error")
                .and_then(|v| v.as_str())
                .map(str::to_string),
        })
    }

    /// Tear the session down. Other classes (sharing the same Client)
    /// keep running.
    pub(crate) async fn shutdown(&self) -> Result<(), WorkerError> {
        self.session.destroy().await?;
        Ok(())
    }
}

#[async_trait]
impl AgentWorker for BatchWorker {
    async fn submit(&self, job: JobInput) -> Result<JobOutput, WorkerError> {
        Self::submit(self, job).await
    }

    fn class(&self) -> WorkerClass {
        Self::class(self)
    }
}

/// Find the first balanced JSON object in `text`. Models sometimes wrap
/// the JSON in chatter despite our instructions; we lift it out.
fn extract_json(text: &str) -> Option<serde_json::Value> {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut start: Option<usize> = None;
    let mut in_string = false;
    let mut escaped = false;

    for (i, &b) in bytes.iter().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'{' => {
                if depth == 0 {
                    start = Some(i);
                }
                depth += 1;
            }
            b'}' if depth > 0 => {
                depth -= 1;
                if depth == 0
                    && let Some(s) = start
                    && let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes[s..=i])
                {
                    return Some(v);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "tests panic on precondition failures")]
mod tests {
    use super::*;

    #[test]
    fn extract_json_finds_balanced_object() {
        let s = r#"Here is the answer: {"job_id":"abc","output":{"goals":[]}} done."#;
        let v = extract_json(s).unwrap();
        assert_eq!(v["job_id"], "abc");
    }

    #[test]
    fn extract_json_handles_nested_braces() {
        let s = r#"{"a": {"b": {"c": 1}}}"#;
        let v = extract_json(s).unwrap();
        assert_eq!(v["a"]["b"]["c"], 1);
    }

    #[test]
    fn extract_json_handles_strings_with_braces() {
        let s = r#"{"text":"this } is fine"}"#;
        let v = extract_json(s).unwrap();
        assert_eq!(v["text"], "this } is fine");
    }

    #[test]
    fn extract_json_returns_none_when_unbalanced() {
        let s = "oops {{{";
        assert!(extract_json(s).is_none());
    }

    #[test]
    fn extract_json_handles_escaped_quotes() {
        let s = r#"{"q":"she said \"hi\""}"#;
        let v = extract_json(s).unwrap();
        assert_eq!(v["q"], "she said \"hi\"");
    }
}
