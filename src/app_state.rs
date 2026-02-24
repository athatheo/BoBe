use arc_swap::ArcSwap;
use reqwest::Client;
use sqlx::sqlite::SqlitePool;
use std::sync::Arc;

use crate::util::capture::ScreenCapture;
use crate::llm::ollama_manager::OllamaManager;
use crate::util::network::MdnsAnnouncer;
use crate::util::sse::connection_manager::SseConnectionManager;
use crate::util::sse::event_queue::EventQueue;
use crate::tools::mcp::adapter::McpToolAdapter;
use crate::tools::registry::ToolRegistry;
use crate::runtime::learning::LearningLoop;
use crate::runtime::session::RuntimeSession;
use crate::services::context_assembler::ContextAssembler;
use crate::services::conversation_service::ConversationService;
use crate::services::goal_worker::ask_user::AskUserBridge;
use crate::services::goals::goals_service::GoalsService;
use crate::config_manager::ConfigManager;
use crate::config::Config;
use crate::llm::EmbeddingProvider;
use crate::llm::LlmProvider;
use crate::db::AgentJobRepository;
use crate::db::ConversationRepository;
use crate::db::CooldownRepository;
use crate::db::GoalPlanRepository;
use crate::db::GoalRepository;
use crate::db::LearningStateRepository;
use crate::db::McpConfigRepository;
use crate::db::MemoryRepository;
use crate::db::ObservationRepository;
use crate::db::SoulRepository;
use crate::db::UserProfileRepository;

/// Shared application state passed through Axum extractors.
#[allow(dead_code)]
pub struct AppState {
    pub db: SqlitePool,
    pub config: Arc<ArcSwap<Config>>,
    pub http_client: Client,
    pub event_queue: Arc<EventQueue>,
    pub connection_manager: Arc<SseConnectionManager>,
    pub llm_provider: Arc<dyn LlmProvider>,
    pub vision_llm_provider: Option<Arc<dyn LlmProvider>>,
    pub embedding_provider: Arc<dyn EmbeddingProvider>,
    // Repos
    pub conversation_repo: Arc<dyn ConversationRepository>,
    pub memory_repo: Arc<dyn MemoryRepository>,
    pub goal_repo: Arc<dyn GoalRepository>,
    pub observation_repo: Arc<dyn ObservationRepository>,
    pub cooldown_repo: Arc<dyn CooldownRepository>,
    pub learning_state_repo: Arc<dyn LearningStateRepository>,
    pub agent_job_repo: Arc<dyn AgentJobRepository>,
    pub mcp_config_repo: Arc<dyn McpConfigRepository>,
    pub soul_repo: Arc<dyn SoulRepository>,
    pub user_profile_repo: Arc<dyn UserProfileRepository>,
    pub goal_plan_repo: Arc<dyn GoalPlanRepository>,
    // Services
    pub conversation_service: Arc<ConversationService>,
    pub context_assembler: Arc<ContextAssembler>,
    pub goals_service: Arc<GoalsService>,
    pub tool_registry: Arc<ToolRegistry>,
    pub runtime_session: Arc<RuntimeSession>,
    pub learning_loop: Option<Arc<LearningLoop>>,
    // Infrastructure
    pub screen_capture: Arc<ScreenCapture>,
    pub ollama_manager: Arc<OllamaManager>,
    pub config_manager: Arc<ConfigManager>,
    pub mcp_tool_adapter: Option<Arc<McpToolAdapter>>,
    pub ask_user_bridge: Arc<AskUserBridge>,
    pub mdns_announcer: Arc<MdnsAnnouncer>,
}

impl AppState {
    pub fn config(&self) -> arc_swap::Guard<Arc<Config>> {
        self.config.load()
    }
}
