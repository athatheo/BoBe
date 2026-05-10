//! `ClientHandle` — single shared `github_copilot_sdk::Client` for the
//! entire daemon. Lazy-started: the CLI process spawns on first use, not
//! at boot. All `Session`s in the daemon share this one CLI.
//!
//! Lifecycle:
//!
//! ```text
//!   ClientHandle::new(config_arc)   // bookkeeping only
//!         ↓
//!   .ensure_started()                // first call: build ClientOptions from
//!         ↓                          // current Config snapshot, Client::start
//!   ...
//!         ↓
//!   .stop()                          // Client::stop, drains the CLI process
//! ```
//!
//! On engine config change (cloud↔local toggle, offline flag, etc.) the
//! `WorkerRegistry::reload()` calls `stop()`, which clears the inner
//! `Option`. The next `ensure_started()` rebuilds against the new
//! `Config`. The session-level `model` / `provider` overrides are passed
//! per-session via `SessionConfig` — see `registry.rs::create_or_resume`.

use std::ffi::OsString;
use std::sync::Arc;

use arc_swap::ArcSwap;
use github_copilot_sdk::{Client, ClientOptions};
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::config::Config;
use crate::error::AppError;

pub(crate) struct ClientHandle {
    inner: Mutex<Option<Arc<Client>>>,
    config: Arc<ArcSwap<Config>>,
}

impl ClientHandle {
    pub(crate) fn new(config: Arc<ArcSwap<Config>>) -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(None),
            config,
        })
    }

    /// Spawn the CLI process if not already running, return a handle to
    /// the `Client`. Concurrent callers serialize through the inner
    /// mutex; only one spawns, the rest wait for the cached `Arc`.
    pub(crate) async fn ensure_started(&self) -> Result<Arc<Client>, AppError> {
        let mut guard = self.inner.lock().await;
        if let Some(client) = guard.as_ref() {
            return Ok(Arc::clone(client));
        }
        let opts = client_options_from_config(&self.config.load());
        info!("starting Copilot SDK client");
        let client = Client::start(opts)
            .await
            .map_err(|e| AppError::Internal(format!("Client::start: {e}")))?;
        let arc = Arc::new(client);
        *guard = Some(Arc::clone(&arc));
        Ok(arc)
    }

    /// Best-effort shutdown of the underlying CLI process. Idempotent —
    /// no-op if the client never started. Clears the cached handle so
    /// the next `ensure_started()` re-spawns.
    pub(crate) async fn stop(&self) {
        let taken = {
            let mut guard = self.inner.lock().await;
            guard.take()
        };
        if let Some(client) = taken {
            match client.stop().await {
                Ok(()) => info!("copilot client stopped"),
                Err(e) => warn!(err = %e, "copilot client stop failed"),
            }
        }
    }
}

/// Build `ClientOptions` from the daemon's live `Config` snapshot.
///
/// Today this only flips `COPILOT_OFFLINE` based on `engine.provider_offline`
/// — every other engine field (model + provider base URL + API key) is set
/// per-session via `SessionConfig::with_model` / `with_provider`, which gives
/// us per-class flexibility without forcing a CLI process restart on a
/// vision-only model swap.
///
/// `COPILOT_OFFLINE` is **only** set when the engine is `local`. In cloud
/// mode the CLI needs GitHub network access, and setting offline=true
/// without a `COPILOT_PROVIDER_BASE_URL` (which we never set process-wide;
/// it's per-session) makes the CLI refuse to start with "Offline mode
/// requires a local model provider."
fn client_options_from_config(config: &Config) -> ClientOptions {
    let mut opts = ClientOptions::default();
    if config.engine.engine == "local" && config.engine.provider_offline {
        opts.env
            .push((OsString::from("COPILOT_OFFLINE"), OsString::from("true")));
    }
    opts
}
