use arc_swap::ArcSwap;
use sqlx::sqlite::SqlitePool;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use tokio::sync::Mutex;

use crate::config::Config;
use crate::config::manager::ConfigManager;
use crate::copilot::memory_file::MemoryFile;
use crate::copilot::registry::WorkerRegistry;
use crate::runtime::session::RuntimeSession;
use crate::secrets::SecretStore;
use crate::services::goals::goals_service::GoalsService;
use crate::services::ollama_install_service::OllamaInstallService;
use crate::services::souls_service::SoulsService;
use crate::services::user_profile_service::UserProfileService;
use crate::util::network::MdnsAnnouncer;
use crate::util::sse::connection_manager::SseConnectionManager;
use crate::util::sse::event_queue::EventQueue;
use crate::voice::engines::VoiceEnginesSnapshot;
use crate::voice::install_service::VoiceInstallService;
use crate::voice::sinks::VoiceSink;
use metrics_exporter_prometheus::PrometheusHandle;

pub(crate) struct AppState {
    pub(crate) db: SqlitePool,
    pub(crate) config: Arc<ArcSwap<Config>>,
    pub(crate) event_queue: Arc<EventQueue>,
    pub(crate) connection_manager: Arc<SseConnectionManager>,
    pub(crate) souls_service: Arc<SoulsService>,
    pub(crate) user_profile_service: Arc<UserProfileService>,
    pub(crate) goals_service: Arc<GoalsService>,
    pub(crate) runtime_session: Arc<RuntimeSession>,
    pub(crate) config_manager: Arc<ConfigManager>,
    pub(crate) mcp_config_lock: Arc<Mutex<()>>,
    pub(crate) mdns_announcer: Arc<MdnsAnnouncer>,
    pub(crate) workers: Arc<WorkerRegistry>,
    pub(crate) memory_file: Arc<MemoryFile>,
    pub(crate) ollama_install: Arc<OllamaInstallService>,
    /// Persistent secret backend. Production uses `KeychainSecretStore`;
    /// the trait exists so deep MCP config materialization in `mcp/config.rs`
    /// stays free-function while handler/service code is DI-friendly.
    pub(crate) secret_store: Arc<dyn SecretStore>,
    /// Daemon-owned voice-model installer. Wizard + Settings call its
    /// HTTP endpoints; legacy `scripts/install-voice-models.sh` is gone.
    pub(crate) voice_install: Arc<VoiceInstallService>,
    /// All voice engines under one ArcSwap so the installer can hot-swap
    /// on completion — no daemon restart. WS handler captures the snapshot
    /// at connect time so a mid-session install doesn't yank engines
    /// from an in-flight turn. Hooks (PreToolUse, PostToolUse) read at
    /// fire time so post-install reloads pick up the new filler library.
    pub(crate) voice_engines: Arc<ArcSwap<VoiceEnginesSnapshot>>,
    /// Voice-turn signal — flipped true by the voice turn-flow while a
    /// voice turn is in flight so `BobeHooks` can branch on tone/filler
    /// behavior. Cleared by the `VoiceTurnFlag` RAII guard at turn end.
    /// Safe under single-flight serialization (UserMessageGuard + chat
    /// submit_lock).
    pub(crate) voice_turn_active: Arc<AtomicBool>,
    /// Single-flight permit for `/voice/stream` connections. Kokoro TTS +
    /// the per-WS Opus encoder are not safe for concurrent feed: two
    /// simultaneous WS handlers would interleave frames and produce
    /// scrambled audio. Set true by voice.rs on accept, cleared on
    /// disconnect via RAII; a second concurrent connect is rejected with
    /// a 409-equivalent close.
    pub(crate) voice_ws_active: Arc<AtomicBool>,
    /// Single-slot voice sink that hooks (PreToolUse, ErrorOccurred) read
    /// to push cached filler PCM directly to the active client.
    pub(crate) voice_sink: Arc<VoiceSink>,
    /// In-flight text-turn task counter. The `send_message` handler spawns
    /// detached Tokio tasks to run the LLM stream; without this counter
    /// the graceful-shutdown path closed the DB pool while those tasks
    /// were still mid-`handle_user_message`, panicking sqlx and losing
    /// the assistant turn. Shutdown waits up to 10s for this to reach 0
    /// after aborting the SDK (which makes the in-flight LLM streams
    /// fail fast so tasks complete quickly).
    pub(crate) in_flight_text_turns: Arc<AtomicUsize>,
    /// Prometheus exposition handle. The `/metrics` route calls `.render()`
    /// on each request to produce the text-format snapshot.
    pub(crate) metrics_handle: PrometheusHandle,
}

impl AppState {
    pub(crate) fn config(&self) -> arc_swap::Guard<Arc<Config>> {
        self.config.load()
    }
}
