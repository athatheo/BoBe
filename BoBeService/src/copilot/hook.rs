//! Hook IPC — wire format + per-worker UDS listener.
//!
//! Wire format: one envelope per line, JSON, ≤16 KiB. The shipped
//! `bobe-copilot-hook` binary writes envelopes; this listener reads and
//! routes them by `job_id` to a oneshot a worker is waiting on.
//!
//! Fail-open philosophy: invalid frames log and continue. A stuck or
//! disconnected listener must not break the agent's user task — the
//! envelope sender already exits 0 on send failures.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::{Mutex, oneshot};
use tokio::task::JoinHandle;
use uuid::Uuid;

/// Handle to a running hook listener. Abort to stop the accept loop and
/// release the bound socket FD. Held by `CopilotWorker` so `shutdown`
/// can clean up the daemon-side task.
pub(crate) struct ListenerHandle {
    abort: tokio::task::AbortHandle,
    socket_path: std::path::PathBuf,
}

impl ListenerHandle {
    pub(crate) fn shutdown(self) {
        self.abort.abort();
        // Best-effort: remove the socket file so re-bind on restart is
        // clean even if the kernel hasn't released the bind yet.
        let _rm = std::fs::remove_file(&self.socket_path);
    }
}

const MAX_FRAME_BYTES: usize = 16 * 1024;
#[allow(dead_code, reason = "Phase 1: used by tests + the hook binary's mirror copy")]
pub(crate) const ENVELOPE_VERSION: u32 = 1;

/// Source-of-truth wire envelope. The hook binary builds one of these
/// from Copilot CLI's stdin payload + `BOBE_JOB_ID` env, writes it as
/// one JSON line, and exits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct HookEnvelope {
    pub(crate) version: u32,
    pub(crate) job_id: Option<Uuid>,
    /// Copilot event name, lowercase: `userpromptsubmitted` | `notification`.
    pub(crate) event: String,
    /// Optional human-readable message (Copilot's `notification.message`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) message: Option<String>,
    /// Original Copilot payload, for debugging.
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub(crate) raw: serde_json::Value,
}

/// Routes incoming envelopes to whichever oneshot is currently waiting.
///
/// Each `CopilotWorker` runs serial jobs (per-session mutex), so at most
/// one oneshot is registered at a time. The slot is `Mutex<Option<...>>`
/// rather than a map — keeps the type simple, matches the actual
/// concurrency model.
pub(crate) struct HookRouter {
    pending: Mutex<Option<PendingJob>>,
}

struct PendingJob {
    job_id: Uuid,
    tx: oneshot::Sender<HookEnvelope>,
}

impl HookRouter {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            pending: Mutex::new(None),
        })
    }

    /// Register a oneshot for `job_id`. Returns the receiver side.
    /// Replaces any prior pending registration (caller is supposed to
    /// hold the per-worker submission lock — this only matters if the
    /// caller drops a job mid-flight).
    pub(crate) async fn register(&self, job_id: Uuid) -> oneshot::Receiver<HookEnvelope> {
        let (tx, rx) = oneshot::channel();
        *self.pending.lock().await = Some(PendingJob { job_id, tx });
        rx
    }

    /// Drop the pending registration if it matches `job_id`. Used on
    /// timeout / cancellation to free the slot.
    pub(crate) async fn cancel(&self, job_id: Uuid) {
        let mut guard = self.pending.lock().await;
        if guard.as_ref().is_some_and(|p| p.job_id == job_id) {
            *guard = None;
        }
    }

    async fn dispatch(&self, env: HookEnvelope) {
        let mut guard = self.pending.lock().await;
        let Some(pending) = guard.take() else {
            tracing::debug!(event = %env.event, "hook envelope with no pending job — dropping");
            return;
        };
        // Match by job_id when present; otherwise this is a session-scoped
        // notification and we still resolve the in-flight job (only one
        // job per worker at a time).
        let job_match = env.job_id.is_none_or(|id| id == pending.job_id);
        if !job_match {
            tracing::warn!(
                got = ?env.job_id,
                expected = %pending.job_id,
                "hook envelope job_id mismatch — dropping, restoring pending"
            );
            *guard = Some(pending);
            return;
        }
        // Only deliver on `notification` — `userpromptsubmitted` is a
        // pre-turn signal we don't currently use to resolve jobs.
        if env.event == "notification" {
            // Receiver may be dropped (timeout) — that's fine.
            let _send_result = pending.tx.send(env);
        } else {
            *guard = Some(pending);
        }
    }
}

/// Bind a UDS at `socket_path` and route accepted envelopes through `router`.
/// Caller must hold the returned handle and call `.shutdown()` on it to
/// stop the accept loop and free the socket FD; otherwise the task lives
/// for the daemon's lifetime (matters when workers restart).
pub(crate) async fn spawn_listener(
    socket_path: std::path::PathBuf,
    router: Arc<HookRouter>,
) -> std::io::Result<ListenerHandle> {
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }
    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let listener = UnixListener::bind(&socket_path)?;
    set_owner_only_perms(&socket_path)?;

    tracing::info!(path = %socket_path.display(), "hook listener bound");

    let handle: JoinHandle<()> = tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    let r = Arc::clone(&router);
                    tokio::spawn(async move {
                        if let Err(e) = handle_conn(stream, r).await {
                            tracing::warn!(err = %e, "hook conn failed");
                        }
                    });
                }
                Err(e) => {
                    tracing::warn!(err = %e, "hook accept failed");
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        }
    });
    Ok(ListenerHandle {
        abort: handle.abort_handle(),
        socket_path,
    })
}

async fn handle_conn(
    mut stream: tokio::net::UnixStream,
    router: Arc<HookRouter>,
) -> std::io::Result<()> {
    let mut buf = Vec::with_capacity(2048);
    {
        let (read_half, _) = stream.split();
        let mut reader = BufReader::with_capacity(4096, read_half);
        read_until_bounded(&mut reader, b'\n', &mut buf, MAX_FRAME_BYTES).await?;
    }

    let envelope = match serde_json::from_slice::<HookEnvelope>(&buf) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(err = %e, bytes = buf.len(), "hook envelope parse failed");
            let _write = stream
                .write_all(b"{\"ok\":false,\"error\":\"bad_request\"}\n")
                .await;
            return Ok(());
        }
    };

    tracing::debug!(
        event = %envelope.event,
        job_id = ?envelope.job_id,
        "hook envelope received"
    );

    router.dispatch(envelope).await;
    let _write = stream.write_all(b"{\"ok\":true}\n").await;
    Ok(())
}

async fn read_until_bounded<R: AsyncBufReadExt + Unpin>(
    reader: &mut R,
    delim: u8,
    out: &mut Vec<u8>,
    max: usize,
) -> std::io::Result<()> {
    let mut total = 0usize;
    loop {
        let avail = reader.fill_buf().await?;
        if avail.is_empty() {
            return Ok(());
        }
        let (consume, done) = match avail.iter().position(|&b| b == delim) {
            Some(i) => (i + 1, true),
            None => (avail.len(), false),
        };
        if total + consume > max {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "frame exceeds limit",
            ));
        }
        out.extend_from_slice(&avail[..consume]);
        total += consume;
        reader.consume(consume);
        if done {
            return Ok(());
        }
    }
}

#[allow(unsafe_code, reason = "libc::chmod for UDS perms")]
fn set_owner_only_perms(path: &std::path::Path) -> std::io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let c = CString::new(path.as_os_str().as_bytes())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    // SAFETY: c is a NUL-terminated path; chmod has no aliasing concerns.
    let rc = unsafe { libc::chmod(c.as_ptr(), 0o600) };
    if rc != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

/// Used by tests + future producer paths. Open `path`, write `\n`-framed JSON, close.
#[allow(dead_code, reason = "Phase 1: test-only; daemon-side producers land in Phase 2")]
pub(crate) async fn write_envelope(
    path: &std::path::Path,
    envelope: &HookEnvelope,
) -> std::io::Result<()> {
    let mut stream = tokio::net::UnixStream::connect(path).await?;
    let mut bytes = serde_json::to_vec(envelope)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    bytes.push(b'\n');
    stream.write_all(&bytes).await?;
    stream.shutdown().await?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "tests panic on precondition failures")]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;

    #[tokio::test]
    async fn router_resolves_pending_on_notification() {
        let router = HookRouter::new();
        let id = Uuid::new_v4();
        let rx = router.register(id).await;

        router
            .dispatch(HookEnvelope {
                version: ENVELOPE_VERSION,
                job_id: Some(id),
                event: "notification".into(),
                message: Some("done".into()),
                raw: serde_json::Value::Null,
            })
            .await;

        let env = rx.await.unwrap();
        assert_eq!(env.event, "notification");
    }

    #[tokio::test]
    async fn router_resolves_when_job_id_omitted() {
        let router = HookRouter::new();
        let id = Uuid::new_v4();
        let rx = router.register(id).await;

        router
            .dispatch(HookEnvelope {
                version: ENVELOPE_VERSION,
                job_id: None,
                event: "notification".into(),
                message: None,
                raw: serde_json::Value::Null,
            })
            .await;

        assert!(rx.await.is_ok());
    }

    #[tokio::test]
    async fn router_drops_userpromptsubmitted_keeps_pending() {
        let router = HookRouter::new();
        let id = Uuid::new_v4();
        let rx = router.register(id).await;

        router
            .dispatch(HookEnvelope {
                version: ENVELOPE_VERSION,
                job_id: Some(id),
                event: "userpromptsubmitted".into(),
                message: None,
                raw: serde_json::Value::Null,
            })
            .await;

        // Pending should still be registered; cancel completes cleanly.
        router.cancel(id).await;
        // rx should be dropped/closed because cancel cleared the slot.
        assert!(rx.await.is_err());
    }

    #[tokio::test]
    async fn router_mismatch_keeps_pending() {
        let router = HookRouter::new();
        let mine = Uuid::new_v4();
        let other = Uuid::new_v4();
        let _rx = router.register(mine).await;

        router
            .dispatch(HookEnvelope {
                version: ENVELOPE_VERSION,
                job_id: Some(other),
                event: "notification".into(),
                message: None,
                raw: serde_json::Value::Null,
            })
            .await;

        // Slot should still be occupied by `mine`.
        assert!(router.pending.lock().await.is_some());
    }

    #[tokio::test]
    async fn round_trip_socket() {
        let dir = tempdir();
        let sock = dir.join("hook.sock");
        let router = HookRouter::new();
        spawn_listener(sock.clone(), Arc::clone(&router))
            .await
            .unwrap();

        let id = Uuid::new_v4();
        let rx = router.register(id).await;

        let env = HookEnvelope {
            version: ENVELOPE_VERSION,
            job_id: Some(id),
            event: "notification".into(),
            message: Some("hi".into()),
            raw: serde_json::Value::Null,
        };
        write_envelope(&sock, &env).await.unwrap();

        let got = tokio::time::timeout(std::time::Duration::from_secs(2), rx)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(got.message.as_deref(), Some("hi"));
    }

    #[tokio::test]
    async fn oversized_frame_is_rejected() {
        let dir = tempdir();
        let sock = dir.join("hook.sock");
        let router = HookRouter::new();
        spawn_listener(sock.clone(), Arc::clone(&router))
            .await
            .unwrap();

        let mut stream = tokio::net::UnixStream::connect(&sock).await.unwrap();
        let mut blob = b"{\"version\":1,\"event\":\"notification\",\"message\":\"".to_vec();
        blob.resize(MAX_FRAME_BYTES + 100, b'a');
        blob.extend_from_slice(b"\"}\n");
        stream.write_all(&blob).await.unwrap();
        stream.shutdown().await.unwrap();

        let mut resp = String::new();
        let _read = stream.read_to_string(&mut resp).await;
        // Server closes without sending — frame violation.
        assert!(resp.is_empty() || resp.contains("bad_request"));
    }

    /// macOS UDS paths cap at ~104 chars; the default $TMPDIR
    /// (`/var/folders/.../T/`) plus a uuid blows that easily. Use
    /// `/tmp` + a short suffix.
    fn tempdir() -> std::path::PathBuf {
        let suffix: String = Uuid::new_v4()
            .as_simple()
            .to_string()
            .chars()
            .take(8)
            .collect();
        let d = std::path::PathBuf::from(format!("/tmp/bbh-{suffix}"));
        std::fs::create_dir_all(&d).unwrap();
        d
    }
}
