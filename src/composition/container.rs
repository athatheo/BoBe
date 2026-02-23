use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use arc_swap::ArcSwap;
use reqwest::Client;
use sqlx::sqlite::SqlitePool;

use crate::adapters::capture::ScreenCapture;
use crate::adapters::embedding::LocalEmbeddingProvider;
use crate::adapters::llm::factory::LlmProviderFactory;
use crate::adapters::llm::ollama_manager::OllamaManager;
use crate::adapters::network::MdnsAnnouncer;
use crate::adapters::persistence::repos::agent_job_repo::SqliteAgentJobRepo;
use crate::adapters::persistence::repos::conversation_repo::SqliteConversationRepo;
use crate::adapters::persistence::repos::cooldown_repo::SqliteCooldownRepo;
use crate::adapters::persistence::repos::goal_repo::SqliteGoalRepo;
use crate::adapters::persistence::repos::learning_state_repo::SqliteLearningStateRepo;
use crate::adapters::persistence::repos::mcp_config_repo::SqliteMcpConfigRepo;
use crate::adapters::persistence::repos::memory_repo::SqliteMemoryRepo;
use crate::adapters::persistence::repos::observation_repo::SqliteObservationRepo;
use crate::adapters::persistence::repos::soul_repo::SqliteSoulRepo;
use crate::adapters::persistence::repos::user_profile_repo::SqliteUserProfileRepo;
use crate::adapters::sse::connection_manager::SseConnectionManager;
use crate::adapters::sse::event_queue::EventQueue;
use crate::adapters::tools::native::adapter::NativeToolAdapter;
use crate::adapters::tools::native::base::NativeTool;
use crate::adapters::tools::registry::ToolRegistry;
use crate::application::learners::{
    CaptureLearner, GoalLearner, MemoryConsolidator, MemoryLearner, MessageLearner,
};
use crate::application::learning::{LearningConfig, LearningLoop, RetentionConfig};
use crate::application::runtime::decision_engine::DecisionEngine;
use crate::application::runtime::message_handler::MessageHandler;
use crate::application::runtime::proactive_generator::ProactiveGenerator;
use crate::application::runtime::session::RuntimeSession;
use crate::application::runtime::state::OrchestratorConfig;
use crate::application::services::agent_job_manager::AgentJobManager;
use crate::application::services::context_assembler::ContextAssembler;
use crate::application::services::conversation_service::ConversationService;
use crate::application::services::goals::goals_config::GoalConfig;
use crate::application::services::goals::goals_service::GoalsService;
use crate::application::services::soul_service::SoulService;
use crate::application::triggers::{CheckinScheduler, CheckinTrigger, GoalTrigger};
use crate::application::triggers::agent_job_trigger::AgentJobTrigger;
use crate::application::triggers::capture_trigger::CaptureTrigger;
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

use super::config_manager::ConfigManager;

/// Holds all Arc<dyn Trait> dependencies for the application.
///
/// This is the composition root — the only place that knows all
/// concrete types. Everything else works with trait objects.
pub struct Container {
    pub db: SqlitePool,
    pub config: Arc<ArcSwap<Config>>,
    pub http_client: Client,
    pub llm_provider: Arc<dyn LlmProvider>,
    pub vision_llm_provider: Arc<dyn LlmProvider>,
    pub embedding_provider: Arc<dyn EmbeddingProvider>,
    pub event_queue: Arc<EventQueue>,
    pub connection_manager: Arc<SseConnectionManager>,
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
    pub mdns_announcer: Arc<MdnsAnnouncer>,
    pub config_manager: Arc<ConfigManager>,
    /// Native tool adapter — registered with tool_registry during bootstrap.
    pub native_adapter: Arc<NativeToolAdapter>,
}

impl Container {
    /// Build the container from a config and database pool.
    ///
    /// This wires all concrete implementations to trait objects.
    #[allow(clippy::too_many_lines)]
    pub fn build(
        config: Config,
        pool: SqlitePool,
    ) -> Result<Self, crate::error::AppError> {
        let config_arc = Arc::new(ArcSwap::from_pointee(config.clone()));
        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| crate::error::AppError::Internal(format!("HTTP client build failed: {e}")))?;

        // ── LLM providers ───────────────────────────────────────────────
        let llm_factory = LlmProviderFactory::new(http_client.clone(), config.clone());
        let llm_provider = llm_factory.create(&config.llm_backend)?;
        let vision_llm_provider = llm_factory.create_vision(&config.vision_backend)?;

        // ── Embedding ───────────────────────────────────────────────────
        let embedding_provider: Arc<dyn EmbeddingProvider> = Arc::new(
            LocalEmbeddingProvider::new(
                http_client.clone(),
                &config.ollama_url,
                "nomic-embed-text",
                config.embedding_dimension,
            ),
        );

        // ── SSE ─────────────────────────────────────────────────────────
        let event_queue = Arc::new(EventQueue::new(100));
        let connection_manager = Arc::new(SseConnectionManager::new());

        // ── Repos ───────────────────────────────────────────────────────
        let conversation_repo: Arc<dyn ConversationRepository> =
            Arc::new(SqliteConversationRepo::new(pool.clone()));
        let memory_repo: Arc<dyn MemoryRepository> =
            Arc::new(SqliteMemoryRepo::new(pool.clone()));
        let goal_repo: Arc<dyn GoalRepository> =
            Arc::new(SqliteGoalRepo::new(pool.clone()));
        let observation_repo: Arc<dyn ObservationRepository> =
            Arc::new(SqliteObservationRepo::new(pool.clone()));
        let cooldown_repo: Arc<dyn CooldownRepository> =
            Arc::new(SqliteCooldownRepo::new(pool.clone()));
        let learning_state_repo: Arc<dyn LearningStateRepository> =
            Arc::new(SqliteLearningStateRepo::new(pool.clone()));
        let agent_job_repo: Arc<dyn AgentJobRepository> =
            Arc::new(SqliteAgentJobRepo::new(pool.clone()));
        let mcp_config_repo: Arc<dyn McpConfigRepository> =
            Arc::new(SqliteMcpConfigRepo::new(pool.clone()));
        let soul_repo: Arc<dyn SoulRepository> =
            Arc::new(SqliteSoulRepo::new(pool.clone()));
        let user_profile_repo: Arc<dyn UserProfileRepository> =
            Arc::new(SqliteUserProfileRepo::new(pool.clone()));

        // ── Services ────────────────────────────────────────────────────
        let conversation_service = Arc::new(ConversationService::new(conversation_repo.clone()));

        let soul_service = Arc::new(SoulService::new(
            config.soul_file.as_ref().map(PathBuf::from),
            Some(soul_repo.clone()),
        ));

        let context_assembler = Arc::new(ContextAssembler::new(
            soul_repo.clone(),
            goal_repo.clone(),
            memory_repo.clone(),
            observation_repo.clone(),
            embedding_provider.clone(),
            Some(soul_service),
        ));

        let goal_config = GoalConfig {
            file_path: config.goals_file.as_ref().map(PathBuf::from),
            max_active: config.goals_max_active,
            sync_on_startup: config.goals_sync_on_startup,
            sync_interval_minutes: config.goals_sync_interval_minutes,
        };
        let goals_service = Arc::new(GoalsService::new(
            goal_repo.clone(),
            embedding_provider.clone(),
            goal_config,
        ));

        // ── Tool registry ───────────────────────────────────────────────
        let tool_registry = Arc::new(ToolRegistry::new());
        let native_tools: Vec<Arc<dyn NativeTool>> = vec![
            Arc::new(crate::adapters::tools::native::search_memories::SearchMemoriesTool::new(
                memory_repo.clone(), embedding_provider.clone(),
            )),
            Arc::new(crate::adapters::tools::native::search_context::SearchContextTool::new(
                memory_repo.clone(), embedding_provider.clone(),
            )),
            Arc::new(crate::adapters::tools::native::search_goal::SearchGoalTool::new(
                goal_repo.clone(), embedding_provider.clone(),
            )),
            Arc::new(crate::adapters::tools::native::get_goals::GetGoalsTool::new(
                goal_repo.clone(),
            )),
            Arc::new(crate::adapters::tools::native::get_souls::GetSoulsTool::new(
                soul_repo.clone(),
            )),
            Arc::new(crate::adapters::tools::native::get_recent_context::GetRecentContextTool::new(
                observation_repo.clone(),
            )),
            Arc::new(crate::adapters::tools::native::create_memory::CreateMemoryTool::new(
                memory_repo.clone(), embedding_provider.clone(),
            )),
            Arc::new(crate::adapters::tools::native::update_memory::UpdateMemoryTool::new(
                memory_repo.clone(),
            )),
            Arc::new(crate::adapters::tools::native::create_goal::CreateGoalTool::new(
                goal_repo.clone(), embedding_provider.clone(),
            )),
            Arc::new(crate::adapters::tools::native::update_goal::UpdateGoalTool::new(
                goal_repo.clone(),
            )),
            Arc::new(crate::adapters::tools::native::complete_goal::CompleteGoalTool::new(
                goal_repo.clone(),
            )),
            Arc::new(crate::adapters::tools::native::archive_goal::ArchiveGoalTool::new(
                goal_repo.clone(),
            )),
            Arc::new(crate::adapters::tools::native::file_reader::FileReaderTool::new()),
            Arc::new(crate::adapters::tools::native::list_directory::ListDirectoryTool::new()),
            Arc::new(crate::adapters::tools::native::search_files::SearchFilesTool::new()),
            Arc::new(crate::adapters::tools::native::fetch_url::FetchUrlTool::new()),
            Arc::new(crate::adapters::tools::native::browser_history::BrowserHistoryTool::new()),
            Arc::new(crate::adapters::tools::native::discover_git_repos::DiscoverGitReposTool::new()),
            Arc::new(crate::adapters::tools::native::discover_installed_tools::DiscoverInstalledToolsTool::new()),
            Arc::new(crate::adapters::tools::native::launch_coding_agent::LaunchCodingAgentTool::new(
                agent_job_repo.clone(),
            )),
            Arc::new(crate::adapters::tools::native::check_coding_agent::CheckCodingAgentTool::new(
                agent_job_repo.clone(),
            )),
            Arc::new(crate::adapters::tools::native::cancel_coding_agent::CancelCodingAgentTool::new(
                agent_job_repo.clone(),
            )),
            Arc::new(crate::adapters::tools::native::list_coding_agents::ListCodingAgentsTool::new(
                agent_job_repo.clone(),
            )),
        ];
        let native_adapter = Arc::new(NativeToolAdapter::new(native_tools));
        // Registration happens async in bootstrap after build

        // ── Learners ────────────────────────────────────────────────────
        let learning_config = LearningConfig::from_app_config(&config);

        let message_learner = Arc::new(MessageLearner::new(
            embedding_provider.clone(),
            observation_repo.clone(),
        ));

        let capture_learner = Arc::new(CaptureLearner::new(
            llm_provider.clone(),
            embedding_provider.clone(),
            observation_repo.clone(),
            memory_repo.clone(),
            Some(vision_llm_provider.clone()),
        ));

        let memory_learner = Arc::new(MemoryLearner::new(
            llm_provider.clone(),
            embedding_provider.clone(),
            memory_repo.clone(),
            learning_config.clone(),
        ));

        let goal_learner = Arc::new(GoalLearner::new(
            llm_provider.clone(),
            embedding_provider.clone(),
            goals_service.clone(),
            learning_config.clone(),
        ));

        let memory_consolidator = Arc::new(MemoryConsolidator::new(
            llm_provider.clone(),
            embedding_provider.clone(),
            memory_repo.clone(),
            learning_config.clone(),
        ));

        // ── Orchestrator config ─────────────────────────────────────────
        let orch_config = OrchestratorConfig::from_config(&config);

        // ── Decision engine + proactive generator ───────────────────────
        let decision_engine = Arc::new(DecisionEngine::new(
            llm_provider.clone(),
            observation_repo.clone(),
            conversation_service.clone(),
            orch_config.clone(),
            Some(context_assembler.clone()),
        ));

        let proactive_generator = Arc::new(ProactiveGenerator::new(
            llm_provider.clone(),
            context_assembler.clone(),
            conversation_service.clone(),
            event_queue.clone(),
            config.conversation_summary_enabled,
            Some(cooldown_repo.clone()),
        ));

        // ── Triggers ────────────────────────────────────────────────────
        let _capture_trigger = CaptureTrigger::new(
            capture_learner,
            decision_engine.clone(),
            proactive_generator.clone(),
            Some(cooldown_repo.clone()),
            orch_config.clone(),
        );

        let checkin_scheduler = CheckinScheduler::new(
            &config.checkin_times_vec(),
            config.checkin_interval_minutes,
            config.checkin_jitter_minutes,
            config.checkin_enabled,
        );
        let checkin_trigger = CheckinTrigger::new(
            checkin_scheduler,
            proactive_generator.clone(),
            conversation_service.clone(),
            Some(cooldown_repo.clone()),
            orch_config.clone(),
        );

        let goal_trigger = Arc::new(GoalTrigger::new(
            goal_repo.clone(),
            decision_engine,
            proactive_generator.clone(),
            Some(cooldown_repo.clone()),
            orch_config.clone(),
        ));

        // Agent job trigger (optional)
        let agent_job_trigger = if config.coding_agents_enabled {
            let profiles: HashMap<String, _> = serde_json::from_str(&config.coding_agent_profiles)
                .unwrap_or_default();
            let agent_job_manager = Arc::new(AgentJobManager::new(
                agent_job_repo.clone(),
                profiles,
                PathBuf::from(&config.coding_agent_output_dir),
                config.coding_agent_max_concurrent as usize,
                config.coding_agent_max_runtime_seconds,
            ));
            Some(Arc::new(AgentJobTrigger::new(
                agent_job_manager,
                agent_job_repo.clone(),
                proactive_generator,
                orch_config.clone(),
                Some(llm_provider.clone()),
            )))
        } else {
            None
        };

        // ── RuntimeSession ──────────────────────────────────────────────
        let runtime_session = Arc::new(RuntimeSession::new(
            checkin_trigger,
            goal_trigger,
            Arc::new(MessageHandler::new(
                llm_provider.clone(),
                context_assembler.clone(),
                conversation_service.clone(),
                message_learner,
                Some(cooldown_repo.clone()),
                event_queue.clone(),
                orch_config.clone(),
            )),
            conversation_service.clone(),
            Some(cooldown_repo.clone()),
            event_queue.clone(),
            orch_config,
            agent_job_trigger,
        ));

        // ── LearningLoop (optional) ────────────────────────────────────
        let learning_loop = if config.learning_enabled {
            let retention_config = RetentionConfig::from_app_config(&config);
            Some(Arc::new(LearningLoop::new(
                conversation_service.clone(),
                goals_service.clone(),
                memory_learner,
                goal_learner,
                memory_consolidator,
                memory_repo.clone(),
                observation_repo.clone(),
                goal_repo.clone(),
                learning_state_repo.clone(),
                embedding_provider.clone(),
                learning_config,
                retention_config,
            )))
        } else {
            None
        };

        // ── Capture ─────────────────────────────────────────────────────
        let screen_capture = Arc::new(ScreenCapture::new());

        // ── Ollama manager ──────────────────────────────────────────────
        let ollama_manager = Arc::new(OllamaManager::new(
            http_client.clone(),
            &config.ollama_url,
            &config.ollama_model,
            config.ollama_auto_start,
            config.ollama_auto_pull,
            config.ollama_binary_path.clone(),
        ));

        // ── Network ─────────────────────────────────────────────────────
        let mdns_announcer = Arc::new(MdnsAnnouncer::new(
            config.port,
            config.mdns_enabled && config.host == "0.0.0.0",
        ));

        // ── Config manager ──────────────────────────────────────────────
        let config_manager = Arc::new(ConfigManager::new(config_arc.clone()));

        Ok(Self {
            db: pool,
            config: config_arc,
            http_client,
            llm_provider,
            vision_llm_provider,
            embedding_provider,
            event_queue,
            connection_manager,
            conversation_repo,
            memory_repo,
            goal_repo,
            observation_repo,
            cooldown_repo,
            learning_state_repo,
            agent_job_repo,
            mcp_config_repo,
            soul_repo,
            user_profile_repo,
            conversation_service,
            context_assembler,
            goals_service,
            tool_registry: tool_registry.clone(),
            runtime_session,
            learning_loop,
            screen_capture,
            ollama_manager,
            mdns_announcer,
            config_manager,
            native_adapter,
        })
    }
}
