//! Infrastructure layer — transports and ambient process state.
//!
//! Pre-pivot this also constructed `LlmProvider` / `EmbeddingProvider`
//! providers (Ollama / OpenAI / Azure / llama.cpp), the `OllamaManager`
//! to spawn the local Ollama daemon, and `BinaryManager` to download
//! managed binaries. All of that is gone — Copilot CLI is the engine
//! now, and the SDK manages its own process via the
//! `copilot::client::ClientHandle`.

use std::sync::Arc;

use arc_swap::ArcSwap;
use reqwest::Client;

use crate::config::Config;
use crate::error::AppError;
use crate::util::network::MdnsAnnouncer;
use crate::util::sse::connection_manager::SseConnectionManager;
use crate::util::sse::event_queue::EventQueue;

pub(crate) struct Infrastructure {
    pub(crate) config_arc: Arc<ArcSwap<Config>>,
    pub(crate) http_client: Client,
    pub(crate) event_queue: Arc<EventQueue>,
    pub(crate) connection_manager: Arc<SseConnectionManager>,
    pub(crate) mdns_announcer: Arc<MdnsAnnouncer>,
}

impl Infrastructure {
    pub(crate) fn build(config: &Config) -> Result<Self, AppError> {
        let config_arc = Arc::new(ArcSwap::from_pointee(config.clone()));

        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| AppError::Internal(format!("HTTP client: {e}")))?;

        let event_queue = Arc::new(EventQueue::new(100));
        let connection_manager = Arc::new(SseConnectionManager::new(
            Arc::clone(&event_queue),
            None,
            None,
        ));

        let mdns_announcer = Arc::new(MdnsAnnouncer::new(
            config.server.port,
            config.server.mdns_enabled && config.server.host == "0.0.0.0",
        ));

        Ok(Self {
            config_arc,
            http_client,
            event_queue,
            connection_manager,
            mdns_announcer,
        })
    }
}
