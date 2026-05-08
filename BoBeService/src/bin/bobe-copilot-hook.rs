//! Hook binary spawned by Copilot CLI. Reads JSON from stdin (Copilot's
//! hook payload), wraps it in an `HookEnvelope`, ships it to BoBe's UDS
//! at `$BOBE_HOOK_SOCKET`, exits 0.
//!
//! Fail-open: only stdin-read errors are fatal. Bad JSON / missing
//! socket / send errors print to stderr and exit 0 so the agent's user
//! task never breaks because the daemon is down.

// Hook binary spawned by Copilot CLI. It legitimately writes to stderr
// (operator visibility on fail-open) and never to stdout for daemon
// events — Copilot reads stdout as `hookSpecificOutput` JSON.
#![allow(clippy::print_stderr)]

use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

const ENVELOPE_VERSION: u32 = 1;
const MAX_INPUT_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HookEnvelope {
    version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    job_id: Option<Uuid>,
    event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    raw: serde_json::Value,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            // Fail-open: stderr only, exit 0 so Copilot doesn't break the
            // user's turn. The exception is `StdinFatal` — if we can't
            // even read what Copilot gave us, that's local infrastructure
            // and worth surfacing.
            eprintln!("bobe-copilot-hook: {e}");
            if matches!(e, HookError::StdinFatal(_)) {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum HookError {
    #[error("reading stdin: {0}")]
    StdinFatal(#[source] std::io::Error),

    #[error("payload exceeded {MAX_INPUT_BYTES} bytes")]
    PayloadTooLarge,

    #[error("payload was not valid JSON: {0}")]
    BadJson(#[source] serde_json::Error),

    #[error("BOBE_HOOK_SOCKET not set")]
    SocketMissing,

    #[error("connecting to {path}: {source}")]
    Connect {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("writing envelope: {0}")]
    Write(#[source] std::io::Error),
}

async fn run() -> Result<(), HookError> {
    let raw = read_stdin().map_err(HookError::StdinFatal)?;
    if raw.len() > MAX_INPUT_BYTES {
        return Err(HookError::PayloadTooLarge);
    }
    let payload: serde_json::Value =
        serde_json::from_slice(&raw).map_err(HookError::BadJson)?;

    let event = extract_event(&payload);
    let message = payload
        .get("message")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    // No `job_id` from the hook side — workers run serially per session,
    // so the daemon's HookRouter falls back to the in-flight pending slot
    // (`HookRouter::dispatch` matches `None` to whatever is registered).
    // If parallel jobs per worker are ever needed, set this from a sentinel
    // file the worker writes pre-send_keys.
    let envelope = HookEnvelope {
        version: ENVELOPE_VERSION,
        job_id: None,
        event,
        message,
        raw: payload,
    };

    // For `userPromptSubmitted`, Copilot reads stdout JSON as the
    // `hookSpecificOutput` (see tebis `contrib/copilot/copilot-hook.sh`).
    // Phase 1 doesn't inject context yet — keep stdout clean.

    let socket_path = std::env::var_os("BOBE_HOOK_SOCKET")
        .map(PathBuf::from)
        .ok_or(HookError::SocketMissing)?;

    write_envelope(&socket_path, &envelope).await
}

fn read_stdin() -> std::io::Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(2048);
    std::io::stdin().read_to_end(&mut buf)?;
    Ok(buf)
}

fn extract_event(payload: &serde_json::Value) -> String {
    payload
        .get("hook_event_name")
        .or_else(|| payload.get("eventName"))
        .and_then(|v| v.as_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default()
}

async fn write_envelope(path: &PathBuf, env: &HookEnvelope) -> Result<(), HookError> {
    use tokio::io::AsyncWriteExt;

    let mut stream = tokio::net::UnixStream::connect(path)
        .await
        .map_err(|e| HookError::Connect {
            path: path.clone(),
            source: e,
        })?;

    let mut bytes = serde_json::to_vec(env).map_err(HookError::BadJson)?;
    bytes.push(b'\n');
    stream.write_all(&bytes).await.map_err(HookError::Write)?;
    stream.shutdown().await.map_err(HookError::Write)?;
    Ok(())
}
