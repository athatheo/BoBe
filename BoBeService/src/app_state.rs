use arc_swap::ArcSwap;
use sqlx::sqlite::SqlitePool;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::sync::Mutex;

use crate::config::Config;
use crate::config_manager::ConfigManager;
use crate::copilot::memory_file::MemoryFile;
use crate::copilot::registry::WorkerRegistry;
use crate::db::SoulRepository;
use crate::db::UserProfileRepository;
use crate::runtime::session::RuntimeSession;
use crate::services::goals::goals_service::GoalsService;
use crate::services::ollama_install_service::OllamaInstallService;
use crate::speech::{AcousticVad, SemanticTurn, SttEngine, TtsEngine};
use crate::voice::filler_library::FillerLibrary;
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
    pub(crate) ollama_install: Arc<OllamaInstallService>,
    /// Voice — `None` when sherpa-onnx models aren't installed; `/voice/stream` 503s.
    pub(crate) voice_stt: Option<Arc<dyn SttEngine>>,
    pub(crate) voice_tts: Option<Arc<dyn TtsEngine>>,
    /// Acoustic VAD (Silero v6.2.1 via sherpa-onnx). `None` when model is missing.
    pub(crate) voice_vad: Option<Arc<dyn AcousticVad>>,
    /// Semantic turn-detection. Stub for now (always returns 1.0); real
    /// smart-turn-v3.1 impl wires in with the VAD pipeline.
    pub(crate) voice_smart_turn: Option<Arc<dyn SemanticTurn>>,
    /// Pre-rendered filler PCM library — TTFT watchdog, post-barge-in
    /// recovery, per-tool intents, error reconnect. `None` when TTS engine
    /// isn't loaded; voice still works, just with silence during gaps.
    /// Indexed by `FillerKind` for typed lookups from the handler + hooks.
    pub(crate) voice_filler_library: Option<Arc<FillerLibrary>>,
    /// Voice-turn signal — flipped true by `voice.rs` while a voice turn is
    /// in flight so `BobeHooks` can branch on tone/filler behavior. Cleared
    /// via the guard in `voice.rs::process_turn`. Safe under single-flight
    /// serialization (UserMessageGuard + chat submit_lock).
    pub(crate) voice_turn_active: Arc<AtomicBool>,
}

impl AppState {
    pub(crate) fn config(&self) -> arc_swap::Guard<Arc<Config>> {
        self.config.load()
    }
}
