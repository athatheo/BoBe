use arc_swap::ArcSwap;
use reqwest::Client;
use sqlx::sqlite::SqlitePool;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::binary_manager::BinaryManager;
use crate::config::Config;
use crate::config_manager::ConfigManager;
use crate::copilot::memory_file::MemoryFile;
use crate::copilot::registry::WorkerRegistry;
use crate::db::AgentJobRepository;
use crate::db::ConversationRepository;
use crate::db::CooldownRepository;
use crate::db::LearningStateRepository;
use crate::db::ObservationRepository;
use crate::db::SoulRepository;
use crate::db::UserProfileRepository;
use crate::llm::EmbeddingProvider;
use crate::llm::LlmProvider;
use crate::llm::ollama_manager::OllamaManager;
use crate::runtime::session::RuntimeSession;
use crate::services::conversation_service::ConversationService;
use crate::services::goals::goals_service::GoalsService;
use crate::tools::mcp::adapter::McpToolAdapter;
use crate::util::capture::ScreenCapture;
use crate::util::network::MdnsAnnouncer;
use crate::util::sse::connection_manager::SseConnectionManager;
use crate::util::sse::event_queue::EventQueue;

/// Shared application state (Axum `State` extractor).
#[allow(dead_code)]
pub(crate) struct AppState {
    pub(crate) db: SqlitePool,
    pub(crate) config: Arc<ArcSwap<Config>>,
    pub(crate) http_client: Client,
    pub(crate) event_queue: Arc<EventQueue>,
    pub(crate) connection_manager: Arc<SseConnectionManager>,
    pub(crate) llm_provider: Arc<dyn LlmProvider>,
    pub(crate) vision_llm_provider: Option<Arc<dyn LlmProvider>>,
    pub(crate) embedding_provider: Arc<dyn EmbeddingProvider>,
    pub(crate) conversation_repo: Arc<dyn ConversationRepository>,
    pub(crate) observation_repo: Arc<dyn ObservationRepository>,
    pub(crate) cooldown_repo: Arc<dyn CooldownRepository>,
    pub(crate) learning_state_repo: Arc<dyn LearningStateRepository>,
    pub(crate) agent_job_repo: Arc<dyn AgentJobRepository>,
    pub(crate) soul_repo: Arc<dyn SoulRepository>,
    pub(crate) user_profile_repo: Arc<dyn UserProfileRepository>,
    pub(crate) conversation_service: Arc<ConversationService>,
    pub(crate) goals_service: Arc<GoalsService>,
    pub(crate) runtime_session: Arc<RuntimeSession>,
    pub(crate) screen_capture: Arc<ScreenCapture>,
    pub(crate) ollama_manager: Arc<OllamaManager>,
    pub(crate) binary_manager: Arc<BinaryManager>,
    pub(crate) config_manager: Arc<ConfigManager>,
    pub(crate) mcp_tool_adapter: Option<Arc<McpToolAdapter>>,
    pub(crate) mcp_config_lock: Arc<Mutex<()>>,
    pub(crate) mdns_announcer: Arc<MdnsAnnouncer>,
    /// Copilot CLI worker fleet — lazy-spawned per worker class.
    /// Phase 2 onwards consumers go through this for any LLM-ish work.
    pub(crate) workers: Arc<WorkerRegistry>,
    /// Single-writer to `~/.bobe/memory.md`. All BoBe writes funnel
    /// through this so workers (which only read via the symlink in
    /// each `<worker_dir>/.github/copilot-instructions.md`) never see
    /// torn content.
    pub(crate) memory_file: Arc<MemoryFile>,
}

impl AppState {
    pub(crate) fn config(&self) -> arc_swap::Guard<Arc<Config>> {
        self.config.load()
    }
}
