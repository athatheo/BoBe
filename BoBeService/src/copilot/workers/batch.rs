//! Our own submit mutex (beyond the SDK's idle_waiter) preserves strict job ordering.

use std::sync::Arc;

use github_copilot_sdk::session::Session;
use github_copilot_sdk::types::MessageOptions;
use tokio::sync::{Mutex, RwLock};

use crate::copilot::error::WorkerError;
use crate::copilot::types::{JobInput, JobOutput, WorkerClass};

pub(crate) struct BatchWorker {
    class: WorkerClass,
    session: Arc<Session>,
    submit_lock: Mutex<()>,
    lifecycle: Arc<RwLock<()>>,
}

impl BatchWorker {
    pub(crate) fn new(
        class: WorkerClass,
        session: Arc<Session>,
        lifecycle: Arc<RwLock<()>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            class,
            session,
            submit_lock: Mutex::new(()),
            lifecycle,
        })
    }

    pub(crate) async fn submit(&self, job: JobInput) -> Result<JobOutput, WorkerError> {
        let _lifecycle = self.lifecycle.read().await;
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

        // Session mode set on SessionConfig in registry; MessageOptions::with_mode is delivery-only.
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

    pub(crate) async fn shutdown(&self) -> Result<(), WorkerError> {
        self.session.disconnect().await?;
        Ok(())
    }

    pub(crate) async fn abort(&self) -> Result<(), WorkerError> {
        self.session.abort().await?;
        Ok(())
    }
}

/// Scan-to-end picks real answer past any schema-example preamble in the prompt.
fn extract_json(text: &str) -> Option<serde_json::Value> {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut start: Option<usize> = None;
    let mut in_string = false;
    let mut escaped = false;

    let mut candidates: Vec<serde_json::Value> = Vec::new();

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
                    candidates.push(v);
                }
            }
            _ => {}
        }
    }

    if candidates.is_empty() {
        return None;
    }

    if let Some(answer) = candidates
        .iter()
        .rev()
        .find(|v| v.get("job_id").is_some() || v.get("output").is_some())
    {
        return Some(answer.clone());
    }

    candidates.pop()
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

    #[test]
    fn extract_json_prefers_canonical_answer_after_example() {
        let s = r#"
            I'll output {"decision":"<one of reach_out, idle, need_more_info>", "reasoning":"<why>"}.
            Here's my answer: {"job_id":"abc","output":{"decision":"reach_out","reasoning":"user looks stuck"}}
        "#;
        let v = extract_json(s).unwrap();
        assert_eq!(v["job_id"], "abc");
        assert_eq!(v["output"]["decision"], "reach_out");
    }

    #[test]
    fn extract_json_falls_back_to_last_when_no_canonical_keys() {
        let s = r#"Notes: {"foo":1} Result: {"bar":2}"#;
        let v = extract_json(s).unwrap();
        assert_eq!(v["bar"], 2);
    }
}
