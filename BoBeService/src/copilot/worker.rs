//! `CopilotWorker` — one `github_copilot_sdk::session::Session`, one job at a
//! time. The SDK owns the underlying Copilot CLI server process (shared via
//! the `Client` held by the registry); this struct owns conversation state
//! for one worker class.
//!
//! Submission model: `submit` builds a single prompt, calls
//! `session.send_and_wait(opts)` which blocks until `session.idle` and
//! returns the final `assistant.message` event. We extract the content,
//! parse the first balanced JSON object out of it, and surface the result.
//!
//! Permission auto-approval and memory.md context injection are configured
//! on the `Session` at creation time (see `registry.rs`).

#![allow(
    dead_code,
    reason = "Phase 2 onwards: Worker public surface; consumers cut over in Phase 5"
)]

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use github_copilot_sdk::session::Session;
use github_copilot_sdk::types::MessageOptions;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

use super::agent_worker::AgentWorker;

/// Default per-turn deadline. Workers can override via `WorkerConfig`.
pub(crate) const DEFAULT_TURN_TIMEOUT: Duration = Duration::from_mins(2);

#[derive(Debug, thiserror::Error)]
pub(crate) enum WorkerError {
    #[error("copilot SDK error: {0}")]
    Sdk(#[from] github_copilot_sdk::Error),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("turn timed out after {}s", .0.as_secs())]
    Timeout(Duration),

    #[error("worker returned no parseable JSON output for job {0}")]
    NoJsonOutput(Uuid),

    #[error("worker returned no assistant.message event for job {0}")]
    NoAssistantMessage(Uuid),
}

/// Worker job request. `instructions` is the freeform prompt the worker sees;
/// `kind` is a tag for tracing; `input` is opaque payload the prompt may
/// reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct JobInput {
    pub(crate) job_id: Uuid,
    pub(crate) kind: String,
    pub(crate) instructions: String,
    #[serde(default)]
    pub(crate) input: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct JobOutput {
    pub(crate) job_id: Uuid,
    #[serde(default)]
    pub(crate) output: serde_json::Value,
    #[serde(default)]
    pub(crate) text: String,
    #[serde(default)]
    pub(crate) error: Option<String>,
}

pub(crate) struct WorkerConfig {
    pub(crate) name: String,
    pub(crate) turn_timeout: Duration,
}

pub(crate) struct CopilotWorker {
    name: String,
    session: Arc<Session>,
    /// Serializes submits per worker — only one job in flight at a time.
    /// The SDK's idle_waiter slot is also a single-flight gate, but our
    /// callers expect strict job ordering so we own the gate.
    submit_lock: Mutex<()>,
    turn_timeout: Duration,
}

impl CopilotWorker {
    pub(crate) fn new(session: Arc<Session>, cfg: WorkerConfig) -> Arc<Self> {
        Arc::new(Self {
            name: cfg.name,
            session,
            submit_lock: Mutex::new(()),
            turn_timeout: cfg.turn_timeout,
        })
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

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

        let opts = MessageOptions::new(prompt).with_wait_timeout(self.turn_timeout);

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

    /// Tear the session down. Other workers (sharing the same Client) keep
    /// running.
    pub(crate) async fn shutdown(&self) -> Result<(), WorkerError> {
        self.session.destroy().await?;
        Ok(())
    }
}

#[async_trait]
impl AgentWorker for CopilotWorker {
    async fn submit(&self, job: JobInput) -> Result<JobOutput, WorkerError> {
        Self::submit(self, job).await
    }

    fn name(&self) -> &str {
        Self::name(self)
    }
}

/// Find the first balanced JSON object in `text`. Models sometimes wrap the
/// JSON in chatter despite our instructions; we lift it out.
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
            b'}' => {
                if depth > 0 {
                    depth -= 1;
                    if depth == 0
                        && let Some(s) = start
                        && let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes[s..=i])
                    {
                        return Some(v);
                    }
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
