//! In-process driver for `copilot login` (GitHub's official Copilot CLI
//! device-flow). The daemon spawns the installed CLI under a pseudo-TTY
//! (macOS `script -q /dev/null …`) because the Node CLI suppresses output
//! when `process.stdout.isTTY` is false, parses the device code + URL
//! from stdout, and broadcasts phase updates to one or more SSE consumers.
//!
//! Auth ownership: the **CLI**, not BoBe. The Copilot CLI's own approved
//! OAuth client_id handles Individual + Business + Enterprise + SSO; the
//! token is stored in macOS Keychain by the CLI itself. BoBe never sees
//! the token — subsequent SDK spawns (via `--no-auto-login` omitted in
//! `client_options_from_config`) pick it up from the Keychain
//! automatically.
//!
//! Single-flight: at most one in-flight `copilot login` per daemon. A
//! concurrent start returns `Conflict`.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{Mutex, oneshot, watch};
use tracing::{info, warn};

use crate::error::AppError;

#[allow(
    clippy::expect_used,
    reason = "static compile-time regexes; failure is a developer error caught in tests"
)]
static CODE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b([A-Z0-9]{4}-[A-Z0-9]{4})\b").expect("static device-code regex")
});

#[allow(
    clippy::expect_used,
    reason = "static compile-time regexes; failure is a developer error caught in tests"
)]
static URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"https?://[^\s]+/login/device").expect("static device-flow URL regex")
});

/// Phase as exposed to the UI. Mirrors `LoginEvent` in the Swift client;
/// keep them in sync. `tag = "phase"` flattens the wire shape so the
/// Swift side decodes via a single `phase` discriminator.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case", tag = "phase")]
pub(crate) enum LoginPhase {
    /// Coordinator created the child but hasn't seen the device-flow prompt yet.
    Preparing,
    /// We've parsed the verification URL + user code from CLI stdout.
    AwaitingUser { url: String, code: String },
    /// CLI has printed "Waiting for authorization..." — user is at GitHub.com.
    Polling { url: String, code: String },
    /// CLI exited 0 and token is in Keychain.
    Completed,
    /// CLI exited non-zero or we detected a known error line.
    Failed { message: String },
    /// User clicked Cancel; coordinator killed the child.
    Canceled,
}

impl LoginPhase {
    fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed { .. } | Self::Canceled)
    }
}

/// Holds the in-flight session metadata. The `watch` channel is the right
/// primitive here (vs `broadcast`): late-joining subscribers should see
/// the *current* phase immediately, not just future transitions — so a
/// user who opens the sheet, closes it, and reopens it sees the live
/// code rather than a blank screen until the next CLI line lands.
pub(crate) struct LoginCoordinator {
    inner: Mutex<Option<Session>>,
    on_completed: Arc<dyn Fn() + Send + Sync>,
}

struct Session {
    /// Cancel signal to the watcher task. `take()`'d by the watcher on
    /// receipt; `cancel()` calls `take()` to avoid double-fire.
    cancel_tx: Option<oneshot::Sender<()>>,
    /// Current phase. Cloned on subscribe; UI uses `tokio::sync::watch`
    /// semantics to read the latest value immediately.
    phase: watch::Sender<LoginPhase>,
}

impl LoginCoordinator {
    pub(crate) fn new(on_completed: Arc<dyn Fn() + Send + Sync>) -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(None),
            on_completed,
        })
    }

    /// Returns a watch receiver wired to the in-flight session, starting
    /// one if none exists. Conflict if a session is in-flight in any
    /// non-terminal phase.
    ///
    /// `cli_path` is the helper path already resolved for the SDK client.
    /// The caller starts the client first so external and embedded-fallback
    /// resolution follows the same lifecycle.
    pub(crate) async fn start(
        self: &Arc<Self>,
        cli_path: PathBuf,
    ) -> Result<watch::Receiver<LoginPhase>, AppError> {
        let mut guard = self.inner.lock().await;
        if let Some(existing) = guard.as_ref()
            && !existing.phase.borrow().is_terminal()
        {
            return Err(AppError::Conflict(
                "another sign-in is already in progress".to_string(),
            ));
        }

        let (phase_tx, phase_rx) = watch::channel(LoginPhase::Preparing);
        let (cancel_tx, cancel_rx) = oneshot::channel::<()>();

        *guard = Some(Session {
            cancel_tx: Some(cancel_tx),
            phase: phase_tx.clone(),
        });
        drop(guard);

        let on_completed = Arc::clone(&self.on_completed);
        tokio::spawn(async move {
            if run_login(cli_path, phase_tx, cancel_rx).await {
                on_completed();
            }
            // Session stays in place so `subscribe()` returns the terminal
            // phase to late-joiners; a future `start()` replaces it.
        });

        Ok(phase_rx)
    }

    /// Best-effort cancel: fires the cancel oneshot if still armed. The
    /// watcher kills the child and emits `Canceled`. Idempotent.
    pub(crate) async fn cancel(&self) {
        let mut guard = self.inner.lock().await;
        let Some(session) = guard.as_mut() else {
            return;
        };
        if session.phase.borrow().is_terminal() {
            return;
        }
        if let Some(tx) = session.cancel_tx.take() {
            let _ignored = tx.send(());
        }
    }

    /// Returns a receiver for the current session, or None if no session
    /// has been started this process. Used by the SSE handler to
    /// late-join (e.g., user closed the sheet and reopened it).
    pub(crate) async fn subscribe(&self) -> Option<watch::Receiver<LoginPhase>> {
        let guard = self.inner.lock().await;
        guard.as_ref().map(|s| s.phase.subscribe())
    }
}

/// Watcher task: owns the Child + stdout reader, races stdout reads
/// against the cancel oneshot, and sets the terminal phase on the
/// watch channel.
async fn run_login(
    cli_path: PathBuf,
    phase_tx: watch::Sender<LoginPhase>,
    mut cancel_rx: oneshot::Receiver<()>,
) -> bool {
    // Spawn `script -q /dev/null <cli> login` so the Node CLI sees a PTY
    // on stdout. Without this the CLI's `process.stdout.isTTY` check
    // short-circuits the prompts and we'd never get the device code.
    // `/usr/bin/script` (BSD flavor) is on every Mac since OS X.
    let spawn_result = Command::new("/usr/bin/script")
        .arg("-q")
        .arg("/dev/null")
        .arg(&cli_path)
        // The signed app helper is immutable; updates arrive with BoBe/Sparkle.
        .arg("--no-auto-update")
        .arg("login")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .kill_on_drop(true)
        .spawn();

    let mut child = match spawn_result {
        Ok(c) => c,
        Err(e) => {
            let _ignored = phase_tx.send(LoginPhase::Failed {
                message: format!("spawn {} failed: {e}", cli_path.display()),
            });
            return false;
        }
    };

    let Some(stdout) = child.stdout.take() else {
        let _ignored = phase_tx.send(LoginPhase::Failed {
            message: "internal: child stdout unavailable".to_string(),
        });
        // Best-effort wait so we don't leak; kill_on_drop will also fire.
        let _ignored = child.wait().await;
        return false;
    };
    let mut reader = BufReader::new(stdout).lines();

    let mut captured_url: Option<String> = None;
    let mut captured_code: Option<String> = None;
    let mut saw_polling = false;
    let mut last_line: Option<String> = None;
    let mut canceled = false;
    let mut eof = false;

    while !eof {
        tokio::select! {
            line_res = reader.next_line() => {
                match line_res {
                    Ok(Some(line)) => {
                        let line = line.trim().to_string();
                        if line.is_empty() {
                            continue;
                        }
                        tracing::debug!(line = %line, "copilot_login: cli stdout");
                        last_line = Some(line.clone());

                        if captured_url.is_none()
                            && let Some(m) = URL_RE.find(&line)
                        {
                            captured_url = Some(m.as_str().to_string());
                        }
                        if captured_code.is_none()
                            && let Some(c) = CODE_RE.captures(&line).and_then(|c| c.get(1))
                        {
                            captured_code = Some(c.as_str().to_string());
                        }
                        if !saw_polling && line.to_ascii_lowercase().contains("waiting for authorization") {
                            saw_polling = true;
                        }
                        publish_phase(&phase_tx, captured_url.as_deref(), captured_code.as_deref(), saw_polling);
                    }
                    Ok(None) => { eof = true; }
                    Err(e) => {
                        warn!(err = %e, "copilot_login: stdout read error");
                        eof = true;
                    }
                }
            }
            _ = &mut cancel_rx => {
                canceled = true;
                // Kill the child; wait below will reap. `kill_on_drop`
                // covers the case where this `.kill()` itself errors.
                if let Err(e) = child.kill().await {
                    warn!(err = %e, "copilot_login: kill failed; relying on kill_on_drop");
                }
                break;
            }
        }
    }

    // Drain any remaining buffered output (kill_on_drop closes stdout).
    if canceled {
        let _ignored = phase_tx.send(LoginPhase::Canceled);
    }

    let status = match child.wait().await {
        Ok(s) => s,
        Err(e) => {
            warn!(err = %e, "copilot_login: wait failed");
            if !canceled && !phase_tx.borrow().is_terminal() {
                let _ignored = phase_tx.send(LoginPhase::Failed {
                    message: format!("wait failed: {e}"),
                });
            }
            return false;
        }
    };

    // If we already moved to a terminal phase (cancel raced ahead), stop.
    if phase_tx.borrow().is_terminal() {
        return false;
    }

    if status.success() {
        info!("copilot_login: CLI exited 0; token written to macOS Keychain");
        let _ignored = phase_tx.send(LoginPhase::Completed);
        true
    } else {
        let exit_label = status
            .code()
            .map_or_else(|| "?".to_string(), |c| c.to_string());
        let message = last_line.unwrap_or_else(|| format!("sign-in failed (exit {exit_label})"));
        warn!(?status, %message, "copilot_login: CLI exited non-zero");
        let _ignored = phase_tx.send(LoginPhase::Failed { message });
        false
    }
}

fn publish_phase(
    phase_tx: &watch::Sender<LoginPhase>,
    url: Option<&str>,
    code: Option<&str>,
    saw_polling: bool,
) {
    let (Some(url), Some(code)) = (url, code) else {
        return;
    };
    let next = if saw_polling {
        LoginPhase::Polling {
            url: url.to_string(),
            code: code.to_string(),
        }
    } else {
        LoginPhase::AwaitingUser {
            url: url.to_string(),
            code: code.to_string(),
        }
    };
    // send_if_modified skips wakeups when the value is unchanged so SSE
    // consumers don't see duplicate frames every time the CLI reprints.
    phase_tx.send_if_modified(|cur| {
        if matches_phase(cur, &next) {
            false
        } else {
            *cur = next.clone();
            true
        }
    });
}

fn matches_phase(a: &LoginPhase, b: &LoginPhase) -> bool {
    match (a, b) {
        (
            LoginPhase::AwaitingUser { url: u1, code: c1 },
            LoginPhase::AwaitingUser { url: u2, code: c2 },
        )
        | (LoginPhase::Polling { url: u1, code: c1 }, LoginPhase::Polling { url: u2, code: c2 }) => {
            u1 == u2 && c1 == c2
        }
        (LoginPhase::Preparing, LoginPhase::Preparing)
        | (LoginPhase::Completed, LoginPhase::Completed)
        | (LoginPhase::Canceled, LoginPhase::Canceled) => true,
        (LoginPhase::Failed { message: m1 }, LoginPhase::Failed { message: m2 }) => m1 == m2,
        _ => false,
    }
}
