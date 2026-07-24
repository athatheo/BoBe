//! Handler state. Five sub-contexts so handlers say what they depend on.

use arc_swap::ArcSwap;
use metrics_exporter_prometheus::PrometheusHandle;
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
use crate::services::ollama::install_service::OllamaInstallService;
use crate::services::souls_service::SoulsService;
use crate::services::user_profile_service::UserProfileService;
use crate::util::network::MdnsAnnouncer;
use crate::util::sse::connection_manager::SseConnectionManager;
use crate::util::sse::event_queue::EventQueue;
use crate::voice::engines::VoiceEnginesSnapshot;
use crate::voice::install_service::VoiceInstallService;
use crate::voice::sinks::VoiceSink;

/// Persistence, config, SSE, observability, secrets, HTTP egress.
pub(crate) struct Infrastructure {
    pub(crate) db: SqlitePool,
    pub(crate) config: Arc<ArcSwap<Config>>,
    pub(crate) config_manager: Arc<ConfigManager>,
    pub(crate) event_queue: Arc<EventQueue>,
    pub(crate) connection_manager: Arc<SseConnectionManager>,
    pub(crate) mdns_announcer: Arc<MdnsAnnouncer>,
    /// Trait so `mcp/config.rs` stays free-function while handlers are DI-friendly.
    pub(crate) secret_store: Arc<dyn SecretStore>,
    /// `/metrics` calls `.render()` per request.
    pub(crate) metrics_handle: PrometheusHandle,
    /// Shared so Ollama/voice install + `/models` reuse the connection pool.
    pub(crate) http_client: Arc<reqwest::Client>,
    /// Serializes MCP config writes.
    pub(crate) mcp_config_lock: Arc<Mutex<()>>,
}

/// LLM runtime: state machine, Copilot workers, memory.md writer.
pub(crate) struct RuntimeContext {
    pub(crate) runtime_session: Arc<RuntimeSession>,
    pub(crate) workers: Arc<WorkerRegistry>,
    pub(crate) memory_file: Arc<MemoryFile>,
    /// In-flight detached LLM-stream task count. Shutdown waits up to 10s
    /// for this to reach 0 after aborting the SDK; without it the DB pool
    /// closed while tasks were mid-`handle_user_message`, panicking sqlx.
    pub(crate) in_flight_text_turns: Arc<AtomicUsize>,
}

/// Voice: installer, hot-swap engine snapshot, concurrency flags + hook sink.
pub(crate) struct VoiceContext {
    pub(crate) voice_install: Arc<VoiceInstallService>,
    /// Hot-swapped post-install; WS handler snapshots at connect; hooks read at fire-time.
    pub(crate) voice_engines: Arc<ArcSwap<VoiceEnginesSnapshot>>,
    /// Set true while a voice turn is in flight so hooks can branch on
    /// tone/filler behavior. Cleared by `AtomicFlagGuard` RAII (held in
    /// `voice/run_text_turn.rs::run_text_turn`).
    pub(crate) voice_turn_active: Arc<AtomicBool>,
    /// Single-flight permit for `/voice/stream`. Kokoro + per-WS Opus
    /// encoder aren't concurrent-feed safe; a second connect closes 409.
    pub(crate) voice_ws_active: Arc<AtomicBool>,
    /// Single-slot sink hooks (PreToolUse, ErrorOccurred) push filler PCM into.
    pub(crate) voice_sink: Arc<VoiceSink>,
}

/// File-backed entity stores + Ollama install orchestrator.
pub(crate) struct DomainServices {
    pub(crate) souls_service: Arc<SoulsService>,
    pub(crate) user_profile_service: Arc<UserProfileService>,
    pub(crate) goals_service: Arc<GoalsService>,
    pub(crate) ollama_install: Arc<OllamaInstallService>,
}

/// Auth + sign-in. Distinct from Infrastructure/Services so future auth
/// surfaces (token rotation, alt providers) have a home.
pub(crate) struct AuthContext {
    /// Drives the bundled `copilot login` device flow; single-flight.
    pub(crate) copilot_login: Arc<crate::copilot::login::LoginCoordinator>,
}

pub(crate) struct BodyContext {
    pub(crate) gateway: Arc<crate::body::gateway::BodyGateway>,
}

pub(crate) struct AppState {
    pub(crate) infra: Arc<Infrastructure>,
    pub(crate) runtime: Arc<RuntimeContext>,
    pub(crate) voice: Arc<VoiceContext>,
    pub(crate) services: Arc<DomainServices>,
    pub(crate) auth: Arc<AuthContext>,
    pub(crate) body: Arc<BodyContext>,
}

impl AppState {
    pub(crate) fn config(&self) -> arc_swap::Guard<Arc<Config>> {
        self.infra.config.load()
    }
}
