use std::sync::Arc;
use arc_swap::ArcSwap;
use reqwest::Client;
use sqlx::sqlite::SqlitePool;

use crate::adapters::capture::ScreenCapture;
use crate::adapters::llm::ollama_manager::OllamaManager;
use crate::adapters::sse::connection_manager::SseConnectionManager;
use crate::adapters::sse::event_queue::EventQueue;
use crate::adapters::tools::mcp::adapter::McpToolAdapter;
use crate::adapters::tools::registry::ToolRegistry;
use crate::application::learning::LearningLoop;
use crate::application::runtime::session::RuntimeSession;
use crate::application::services::context_assembler::ContextAssembler;
use crate::application::services::conversation_service::ConversationService;
use crate::application::services::goals::goals_service::GoalsService;
use crate::composition::config_manager::ConfigManager;
use crate::config::Config;
use crate::ports::embedding::EmbeddingProvider;
use crate::ports::llm::LlmProvider;
use crate::ports::repos::agent_job_repo::AgentJobRepository;
use crate::ports::repos::conversation_repo::ConversationRepository;
use crate::ports::repos::cooldown_repo::CooldownRepository;
use crate::ports::repos::goal_repo::GoalRepository;
use crate::ports::repos::learning_state_repo::LearningStateRepository;
use crate::ports::repos::mcp_config_repo::McpConfigRepository;
use crate::ports::repos::memory_repo::MemoryRepository;
use crate::ports::repos::observation_repo::ObservationRepository;
use crate::ports::repos::soul_repo::SoulRepository;
use crate::ports::repos::user_profile_repo::UserProfileRepository;

/// Shared application state passed through Axum extractors.
pub struct AppState {
    pub db: SqlitePool,
    pub config: Arc<ArcSwap<Config>>,
    pub http_client: Client,
    pub event_queue: Arc<EventQueue>,
    pub connection_manager: Arc<SseConnectionManager>,
    pub llm_provider: Arc<dyn LlmProvider>,
    pub vision_llm_provider: Arc<dyn LlmProvider>,
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
}

impl AppState {
    pub fn config(&self) -> arc_swap::Guard<Arc<Config>> {
        self.config.load()
    }
}
