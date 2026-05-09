use arc_swap::ArcSwap;
use sqlx::sqlite::SqlitePool;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config::Config;
use crate::config_manager::ConfigManager;
use crate::copilot::memory_file::MemoryFile;
use crate::copilot::registry::WorkerRegistry;
use crate::db::SoulRepository;
use crate::db::UserProfileRepository;
use crate::runtime::session::RuntimeSession;
use crate::services::goals::goals_service::GoalsService;
use crate::util::capture::ScreenCapture;
use crate::util::network::MdnsAnnouncer;
use crate::util::sse::connection_manager::SseConnectionManager;
use crate::util::sse::event_queue::EventQueue;

pub(crate) struct AppState {
    pub(crate) db: SqlitePool,
    pub(crate) config: Arc<ArcSwap<Config>>,
    pub(crate) event_queue: Arc<EventQueue>,
    pub(crate) connection_manager: Arc<SseConnectionManager>,
    pub(crate) soul_repo: Arc<dyn SoulRepository>,
    pub(crate) user_profile_repo: Arc<dyn UserProfileRepository>,
    pub(crate) goals_service: Arc<GoalsService>,
    pub(crate) runtime_session: Arc<RuntimeSession>,
    pub(crate) screen_capture: Arc<ScreenCapture>,
    pub(crate) config_manager: Arc<ConfigManager>,
    pub(crate) mcp_config_lock: Arc<Mutex<()>>,
    pub(crate) mdns_announcer: Arc<MdnsAnnouncer>,
    pub(crate) workers: Arc<WorkerRegistry>,
    pub(crate) memory_file: Arc<MemoryFile>,
}

impl AppState {
    pub(crate) fn config(&self) -> arc_swap::Guard<Arc<Config>> {
        self.config.load()
    }
}
