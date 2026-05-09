//! Infrastructure layer — transports and ambient process state. The
//! Copilot SDK manages its own CLI process via `copilot::client`, so
//! this layer only owns the SSE event queue, connection manager, mDNS
//! announcer, and the live `Config` arc-swap.

use std::sync::Arc;

use arc_swap::ArcSwap;

use crate::config::Config;
use crate::error::AppError;
use crate::util::network::MdnsAnnouncer;
use crate::util::sse::connection_manager::SseConnectionManager;
use crate::util::sse::event_queue::EventQueue;

pub(crate) struct Infrastructure {
    pub(crate) config_arc: Arc<ArcSwap<Config>>,
    pub(crate) event_queue: Arc<EventQueue>,
    pub(crate) connection_manager: Arc<SseConnectionManager>,
    pub(crate) mdns_announcer: Arc<MdnsAnnouncer>,
}

impl Infrastructure {
    pub(crate) fn build(config: &Config) -> Result<Self, AppError> {
        let config_arc = Arc::new(ArcSwap::from_pointee(config.clone()));

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
            event_queue,
            connection_manager,
            mdns_announcer,
        })
    }
}
