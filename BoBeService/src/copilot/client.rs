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
            match client.stop().await {
                Ok(()) => info!("copilot client stopped"),
                Err(e) => warn!(err = %e, "copilot client stop failed"),
            }
        }
    }
}

/// Only sets `COPILOT_OFFLINE` in local mode; cloud mode requires network access.
fn client_options_from_config(config: &Config) -> ClientOptions {
    let mut opts = ClientOptions::default();
    if config.engine.engine == crate::models::engine_kind::EngineKind::Local
        && config.engine.provider_offline
    {
        opts.env
            .push((OsString::from("COPILOT_OFFLINE"), OsString::from("true")));
    }
    opts
}
