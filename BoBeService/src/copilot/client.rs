use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;
use github_copilot_sdk::{Client, ClientMode, ClientOptions};
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::config::Config;
use crate::error::AppError;

const CLIENT_STOP_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) fn installed_cli_path() -> Option<PathBuf> {
    if let Some(configured) = std::env::var_os("COPILOT_CLI_PATH") {
        let path = PathBuf::from(configured);
        if path.is_file() {
            return Some(path);
        }
        warn!(
            path = %path.display(),
            "COPILOT_CLI_PATH does not point to a file; trying embedded fallback"
        );
    }
    github_copilot_sdk::install_bundled_cli()
}

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

    pub(crate) async fn stop(&self) {
        let taken = {
            let mut guard = self.inner.lock().await;
            guard.take()
        };
        if let Some(client) = taken {
            match tokio::time::timeout(CLIENT_STOP_TIMEOUT, client.stop()).await {
                Ok(Ok(())) => info!("copilot client stopped"),
                Ok(Err(e)) => warn!(err = %e, "copilot client stop completed with errors"),
                Err(_) => {
                    warn!(
                        timeout_ms = CLIENT_STOP_TIMEOUT.as_millis() as u64,
                        "copilot client stop timed out; forcing shutdown"
                    );
                    client.force_stop();
                }
            }
        }
    }

    pub(crate) async fn force_stop(&self) {
        let taken = {
            let mut guard = self.inner.lock().await;
            guard.take()
        };
        if let Some(client) = taken {
            client.force_stop();
            warn!("copilot client force-stopped");
        }
    }
}

/// Only sets `COPILOT_OFFLINE` in local mode; cloud mode requires network access.
fn client_options_from_config(config: &Config) -> ClientOptions {
    let mut opts = ClientOptions::default();
    opts.mode = ClientMode::Empty;
    opts.working_directory = crate::util::paths::bobe_data_dir();
    opts.base_directory = Some(std::env::home_dir().map_or_else(
        || crate::util::paths::bobe_data_dir().join("copilot"),
        |home| home.join(".copilot"),
    ));
    if config.engine.engine == crate::models::engine_kind::EngineKind::Local
        && config.engine.provider_offline
    {
        opts.env
            .push((OsString::from("COPILOT_OFFLINE"), OsString::from("true")));
    }
    opts
}
