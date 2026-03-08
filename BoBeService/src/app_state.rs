use arc_swap::ArcSwap;
use reqwest::Client;
use sqlx::sqlite::SqlitePool;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::binary_manager::BinaryManager;
use crate::config::Config;
use crate::config_manager::ConfigManager;
use crate::db::AgentJobRepository;
use crate::db::ConversationRepository;
use crate::db::CooldownRepository;
use crate::db::GoalPlanRepository;
use crate::db::GoalRepository;
use crate::db::LearningStateRepository;
use crate::db::MemoryRepository;
use crate::db::ObservationRepository;
use crate::db::SoulRepository;
use crate::db::UserProfileRepository;
use crate::llm::EmbeddingProvider;
use crate::llm::LlmProvider;
use crate::llm::ollama_manager::OllamaManager;
use crate::runtime::learning::LearningLoop;
use crate::runtime::session::RuntimeSession;
use crate::services::context_assembler::ContextAssembler;
use crate::services::conversation_service::ConversationService;
use crate::services::goals::goals_service::GoalsService;
use crate::tools::mcp::adapter::McpToolAdapter;
use crate::tools::registry::ToolRegistry;
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
    pub(crate) memory_repo: Arc<dyn MemoryRepository>,
    pub(crate) goal_repo: Arc<dyn GoalRepository>,
    pub(crate) observation_repo: Arc<dyn ObservationRepository>,
    pub(crate) cooldown_repo: Arc<dyn CooldownRepository>,
    pub(crate) learning_state_repo: Arc<dyn LearningStateRepository>,
    pub(crate) agent_job_repo: Arc<dyn AgentJobRepository>,
    pub(crate) soul_repo: Arc<dyn SoulRepository>,
    pub(crate) user_profile_repo: Arc<dyn UserProfileRepository>,
    pub(crate) goal_plan_repo: Arc<dyn GoalPlanRepository>,
    pub(crate) conversation_service: Arc<ConversationService>,
    pub(crate) context_assembler: Arc<ContextAssembler>,
    pub(crate) goals_service: Arc<GoalsService>,
    pub(crate) tool_registry: Arc<ToolRegistry>,
    pub(crate) runtime_session: Arc<RuntimeSession>,
    pub(crate) learning_loop: Option<Arc<LearningLoop>>,
    pub(crate) screen_capture: Arc<ScreenCapture>,
    pub(crate) ollama_manager: Arc<OllamaManager>,
    pub(crate) binary_manager: Arc<BinaryManager>,
    pub(crate) config_manager: Arc<ConfigManager>,
    pub(crate) mcp_tool_adapter: Option<Arc<McpToolAdapter>>,
    pub(crate) mcp_config_lock: Arc<Mutex<()>>,
    pub(crate) mdns_announcer: Arc<MdnsAnnouncer>,
}

impl AppState {
    pub(crate) fn config(&self) -> arc_swap::Guard<Arc<Config>> {
        self.config.load()
    }
}
