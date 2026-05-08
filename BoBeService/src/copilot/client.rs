//! `ClientHandle` — single shared `github_copilot_sdk::Client` for the
//! entire daemon. Lazy-started: the CLI process spawns on first use, not
//! at boot. All `Session`s in the daemon share this one CLI.
//!
//! Lifecycle:
//!
//! ```text
//!   ClientHandle::new()        // bookkeeping only
//!         ↓
//!   .ensure_started()           // first call: Client::start (spawns CLI ~hundreds of ms)
//!         ↓                     // subsequent calls: Arc::clone of cached client
//!   ...
//!         ↓
//!   .stop()                     // Client::stop, drains the CLI process
//! ```

use std::sync::Arc;

use github_copilot_sdk::{Client, ClientOptions};
use tokio::sync::OnceCell;

use crate::error::AppError;

pub(crate) struct ClientHandle {
    inner: OnceCell<Arc<Client>>,
}

impl ClientHandle {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: OnceCell::new(),
        })
    }

    /// Spawn the CLI process if not already running, return a handle to
    /// the `Client`. Concurrent callers race exactly once — the
    /// `OnceCell` serializes initialization.
    pub(crate) async fn ensure_started(&self) -> Result<Arc<Client>, AppError> {
        self.inner
            .get_or_try_init(|| async {
                tracing::info!("starting Copilot SDK client");
                let client = Client::start(client_options())
                    .await
                    .map_err(|e| AppError::Internal(format!("Client::start: {e}")))?;
                Ok::<_, AppError>(Arc::new(client))
            })
            .await
            .cloned()
    }

    /// Best-effort shutdown of the underlying CLI process. Idempotent —
    /// no-op if the client never started.
    pub(crate) async fn stop(&self) {
        if let Some(client) = self.inner.get() {
            match client.stop().await {
                Ok(()) => tracing::info!("copilot client stopped"),
                Err(e) => tracing::warn!(err = %e, "copilot client stop failed"),
            }
        }
    }
}

/// Build `ClientOptions` for BoBe's daemon. Centralized so future tweaks
/// (telemetry, env, transport) land in one place.
fn client_options() -> ClientOptions {
    // Defaults: stdio transport, signed-in-user auth (whoever ran
    // `copilot` interactively). Fields we may want to set later:
    // `with_telemetry(...)` to forward CLI OTel into BoBe's tracing,
    // `with_session_idle_timeout_seconds(...)` to override the default.
    ClientOptions::default()
}
