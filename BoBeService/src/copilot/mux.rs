//! Minimal tmux driver. macOS-only. Internal worker names — no allowlist.
//!
//! Adapted from tebis's `platform::multiplexer`. Per-session mutex keeps
//! `send_keys` text+Enter atomic — cancellation between the two strands
//! characters that prepend to the next turn.

use std::collections::HashMap;
use std::path::Path;
use std::process::{Output, Stdio};
use std::sync::Arc;
use std::time::Duration;

use tokio::process::Command;
use tokio::sync::Mutex as AsyncMutex;

const BINARY: &str = "tmux";
const RUN_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, thiserror::Error)]
pub(crate) enum MuxError {
    #[error("tmux session '{0}' not found")]
    NotFound(String),

    #[error("tmux session '{0}' already exists")]
    AlreadyExists(String),

    #[error("invalid session name: only [A-Za-z0-9._-] allowed, 1..=64 chars")]
    InvalidName,

    #[error("tmux {op} failed: {stderr}")]
    CommandFailed { op: &'static str, stderr: String },

    #[error("tmux {op} timed out ({}s)", RUN_TIMEOUT.as_secs())]
    Timeout { op: &'static str },

    #[error("tmux spawn error: {0}")]
    Spawn(String),
}

pub(crate) type Result<T> = std::result::Result<T, MuxError>;

pub(crate) struct Mux {
    slots: std::sync::Mutex<HashMap<String, Arc<SessionSlot>>>,
    submit_gap: Duration,
}

#[derive(Debug)]
struct SessionSlot {
    exact_target: String,
    lock: AsyncMutex<()>,
}

impl Mux {
    pub(crate) fn new(submit_gap: Duration) -> Self {
        Self {
            slots: std::sync::Mutex::new(HashMap::new()),
            submit_gap,
        }
    }

    pub(crate) async fn has_session(&self, session: &str) -> Result<bool> {
        let slot = self.slot(session)?;
        let out = run("has-session", &["has-session", "-t", &slot.exact_target]).await?;
        Ok(out.status.success())
    }

    pub(crate) async fn new_session(
        &self,
        session: &str,
        cwd: &Path,
        env: &[(&str, &str)],
        command: &[&str],
    ) -> Result<()> {
        let slot = self.slot(session)?;
        let _guard = slot.lock.lock().await;

        let cwd_str = cwd.to_string_lossy();
        let env_pairs: Vec<String> = env.iter().map(|(k, v)| format!("{k}={v}")).collect();

        let mut args: Vec<&str> = vec!["new-session", "-d"];
        for pair in &env_pairs {
            args.push("-e");
            args.push(pair);
        }
        args.push("-s");
        args.push(session);
        args.push("-c");
        args.push(&cwd_str);
        args.extend(command);

        let out = run("new-session", &args).await?;
        classify(&out, "new-session", session)
    }

    /// Send `text` then Enter atomically under the per-session lock.
    pub(crate) async fn send_keys(&self, session: &str, text: &str) -> Result<()> {
        let slot = self.slot(session)?;
        let _guard = slot.lock.lock().await;

        let out = run(
            "send-keys",
            &["send-keys", "-t", &slot.exact_target, "-l", text],
        )
        .await?;
        classify(&out, "send-keys", session)?;

        tokio::time::sleep(self.submit_gap).await;

        // `-H 0d` is literal CR — tmux's spelling for Enter that doesn't
        // require a separate key-name argv parse.
        let out = run(
            "send-keys",
            &["send-keys", "-t", &slot.exact_target, "-H", "0d"],
        )
        .await?;
        classify(&out, "send-keys", session)
    }

    /// Idempotent: NotFound → Ok.
    #[allow(dead_code, reason = "Phase 1: shutdown wiring lands in Phase 2")]
    pub(crate) async fn kill_session(&self, session: &str) -> Result<()> {
        let slot = self.slot(session)?;
        let _guard = slot.lock.lock().await;

        let out = run("kill-session", &["kill-session", "-t", &slot.exact_target]).await?;
        match classify(&out, "kill-session", session) {
            Ok(()) | Err(MuxError::NotFound(_)) => Ok(()),
            Err(e) => Err(e),
        }
    }

    fn slot(&self, session: &str) -> Result<Arc<SessionSlot>> {
        if !is_valid_session_name(session) {
            return Err(MuxError::InvalidName);
        }
        let mut map = self
            .slots
            .lock()
            .map_err(|_| MuxError::Spawn("session slot mutex poisoned".into()))?;
        if let Some(slot) = map.get(session) {
            return Ok(Arc::clone(slot));
        }
        let fresh = Arc::new(SessionSlot {
            exact_target: format!("={session}:0"),
            lock: AsyncMutex::new(()),
        });
        map.insert(session.to_string(), Arc::clone(&fresh));
        Ok(fresh)
    }
}

fn is_valid_session_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

fn classify(out: &Output, op: &'static str, session: &str) -> Result<()> {
    if out.status.success() {
        return Ok(());
    }
    let stderr_lc = String::from_utf8_lossy(&out.stderr).to_ascii_lowercase();

    if stderr_lc.contains("can't find session")
        || stderr_lc.contains("can't find pane")
        || stderr_lc.contains("session not found")
        || stderr_lc.contains("no such session")
    {
        return Err(MuxError::NotFound(session.to_string()));
    }
    if stderr_lc.contains("duplicate session") {
        return Err(MuxError::AlreadyExists(session.to_string()));
    }
    Err(MuxError::CommandFailed {
        op,
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

async fn run(op: &'static str, args: &[&str]) -> Result<Output> {
    let child = Command::new(BINARY)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| MuxError::Spawn(e.to_string()))?;

    match tokio::time::timeout(RUN_TIMEOUT, child.wait_with_output()).await {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(e)) => Err(MuxError::Spawn(format!("tmux io: {e}"))),
        Err(_) => Err(MuxError::Timeout { op }),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "tests assert preconditions via panic")]
mod tests {
    use super::*;

    #[test]
    fn valid_session_names() {
        assert!(is_valid_session_name("bobe-goals"));
        assert!(is_valid_session_name("a"));
        assert!(is_valid_session_name(&"x".repeat(64)));
    }

    #[test]
    fn invalid_session_names() {
        assert!(!is_valid_session_name(""));
        assert!(!is_valid_session_name("with space"));
        assert!(!is_valid_session_name("semi;colon"));
        assert!(!is_valid_session_name(&"x".repeat(65)));
    }

    #[tokio::test]
    async fn slot_caches_per_session() {
        let mux = Mux::new(Duration::from_millis(50));
        let a1 = mux.slot("alpha").unwrap();
        let a2 = mux.slot("alpha").unwrap();
        let b = mux.slot("beta").unwrap();
        assert!(Arc::ptr_eq(&a1, &a2));
        assert!(!Arc::ptr_eq(&a1, &b));
        assert_eq!(a1.exact_target, "=alpha:0");
    }

    #[tokio::test]
    async fn invalid_name_rejected() {
        let mux = Mux::new(Duration::from_millis(50));
        assert!(matches!(
            mux.slot("with space").unwrap_err(),
            MuxError::InvalidName
        ));
    }
}
