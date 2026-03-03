//! Dependency wiring — connects services, tools, learners, triggers, and the
//! runtime session into a coherent application graph.
//!
//! This is the only module that knows all concrete types. Everything it
//! produces is behind `Arc<dyn Trait>` or a concrete `Arc<T>`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tracing::{info, warn};

use crate::config::Config;
use crate::config_manager::ConfigManager;
use crate::runtime::decision_engine::DecisionEngine;
use crate::runtime::learners::{
    CaptureLearner, GoalLearner, MemoryConsolidator, MemoryLearner, MessageLearner,
};
use crate::runtime::learning::LearningLoop;
use crate::runtime::message_handler::MessageHandler;
use crate::runtime::proactive_generator::ProactiveGenerator;
use crate::runtime::session::RuntimeSession;
use crate::runtime::triggers::agent_job_trigger::AgentJobTrigger;
use crate::runtime::triggers::capture_trigger::CaptureTrigger;
use crate::runtime::triggers::{CheckinScheduler, CheckinTrigger, GoalTrigger};
use crate::services::agent_job_manager::AgentJobManager;
use crate::services::context_assembler::ContextAssembler;
use crate::services::conversation_service::ConversationService;
use crate::services::goal_worker::claude_provider::ClaudeAgentProvider;
use crate::services::goal_worker::context_provider::DefaultGoalContextProvider;
use crate::services::goal_worker::manager::GoalWorkerManager;
use crate::services::goal_worker::worker::GoalWorker;
use crate::services::goals::goals_service::GoalsService;
use crate::services::soul_service::SoulService;
use crate::tools::ToolSource;
use crate::tools::executor::ToolExecutor;
use crate::tools::mcp::McpToolAdapter;
use crate::tools::native::adapter::NativeToolAdapter;
use crate::tools::native::base::NativeTool;
use crate::tools::preselector::ToolPreselector;
use crate::tools::registry::ToolRegistry;
use crate::tools::tool_call_loop::ToolCallLoop;
use crate::util::capture::ScreenCapture;
use crate::util::sse::connection_manager::SseConnectionManager;
use crate::util::sse::event_queue::EventQueue;

use super::infra::Infrastructure;
use super::repos::Repositories;

/// Everything produced by wiring, consumed by `AppState` and `main`.
pub struct Wired {
    pub conversation_service: Arc<ConversationService>,
    pub context_assembler: Arc<ContextAssembler>,
    pub goals_service: Arc<GoalsService>,
    pub tool_registry: Arc<ToolRegistry>,
    pub runtime_session: Arc<RuntimeSession>,
    pub learning_loop: Option<Arc<LearningLoop>>,
    pub screen_capture: Arc<ScreenCapture>,
    pub config_manager: Arc<ConfigManager>,
    pub goal_worker_manager: GoalWorkerManager,
    pub mcp_adapter: Arc<McpToolAdapter>,

    // Kept for deferred registration / callback wiring.
    native_adapter: Arc<NativeToolAdapter>,
    agent_job_trigger: Option<Arc<AgentJobTrigger>>,
}

impl Wired {
    /// Register tool sources and wire agent-job callback.
    pub async fn register_tools(&self, config: &Config, _event_queue: &Arc<EventQueue>) {
        self.tool_registry
            .register(self.native_adapter.clone() as Arc<dyn ToolSource>)
            .await;
        info!(
            tools = self.native_adapter.tool_names().len(),
            "bootstrap.native_tools_registered"
        );

        if config.mcp.enabled {
            match self.mcp_adapter.initialize().await {
                Ok(()) => {
                    self.tool_registry
                        .register(self.mcp_adapter.clone() as Arc<dyn ToolSource>)
                        .await;
                    info!("bootstrap.mcp_tools_registered");
                }
                Err(e) => warn!(error = %e, "bootstrap.mcp_init_failed"),
            }
        }

        if let Some(ref trigger) = self.agent_job_trigger {
            trigger.register_callback().await;
        }
    }

    /// Wire SSE connect/disconnect to `RuntimeSession`.
    pub async fn wire_sse_callbacks(&self, cm: &Arc<SseConnectionManager>) {
        let on_connect = {
            let rs = self.runtime_session.clone();
            Box::new(move || {
                let rs = rs.clone();
                tokio::spawn(async move { rs.on_connection().await });
            })
        };
        let on_disconnect = {
            let rs = self.runtime_session.clone();
            Box::new(move || {
                let rs = rs.clone();
                tokio::spawn(async move { rs.on_disconnection().await });
            })
        };
        cm.set_callbacks(on_connect, on_disconnect).await;
        info!("bootstrap.sse_callbacks_wired");
    }
}

// ── Assembly ───────────────────────────────────────────────────────────────

/// Wire all application-level components from infrastructure and repositories.
pub async fn wire(config: &Config, infra: &Infrastructure, repos: &Repositories) -> Wired {
    let config_arc = &infra.config_arc;

    // ── Core services ──────────────────────────────────────────────────
    let conversation_service = Arc::new(ConversationService::new(repos.conversation_repo.clone()));

    let soul_service = Arc::new(SoulService::new(
        config.soul_file.as_ref().map(PathBuf::from),
        Some(repos.soul_repo.clone()),
    ));

    let context_assembler = Arc::new(ContextAssembler::new(
        repos.soul_repo.clone(),
        repos.goal_repo.clone(),
        repos.memory_repo.clone(),
        repos.observation_repo.clone(),
        repos.user_profile_repo.clone(),
        infra.embedding_provider.clone(),
        Some(soul_service),
    ));

    let goals_service = Arc::new(GoalsService::new(
        repos.goal_repo.clone(),
        infra.embedding_provider.clone(),
        config_arc.clone(),
    ));

    // ── Agent job manager (optional) ───────────────────────────────────
    let agent_job_manager = config.coding_agent.enabled.then(|| {
        let profiles: HashMap<String, _> = match serde_json::from_str(&config.coding_agent.profiles)
        {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "wiring.agent_profiles_parse_failed"
                );
                HashMap::new()
            }
        };
        Arc::new(AgentJobManager::new(
            repos.agent_job_repo.clone(),
            profiles,
            PathBuf::from(&config.coding_agent.output_dir),
            config.coding_agent.max_concurrent as usize, // safe: u32→usize on 64-bit
            config.coding_agent.max_runtime_seconds,
        ))
    });

    // ── Tools ──────────────────────────────────────────────────────────
    let tool_registry = Arc::new(ToolRegistry::new());
    let native_adapter = Arc::new(NativeToolAdapter::new(build_native_tools(
        repos,
        &infra.embedding_provider,
        agent_job_manager.as_ref(),
    )));
    let mcp_config_path =
        crate::tools::mcp::config::resolve_mcp_config_path(config.mcp.config_file.as_deref())
            .unwrap_or_else(|e| {
                warn!(
                    error = %e,
                    "wiring.mcp_config_path_resolution_failed"
                );
                PathBuf::from(".bobe/mcp.json")
            });
    let mcp_adapter = Arc::new(McpToolAdapter::new(
        mcp_config_path,
        config.mcp_blocked_commands_vec().to_vec(),
        config.mcp_dangerous_env_keys_vec().to_vec(),
    ));

    // ── Learners ───────────────────────────────────────────────────────
    let message_learner = Arc::new(MessageLearner::new(
        infra.embedding_provider.clone(),
        repos.observation_repo.clone(),
    ));

    let capture_learner = Arc::new(CaptureLearner::new(
        infra.llm_provider.clone(),
        infra.embedding_provider.clone(),
        repos.observation_repo.clone(),
        repos.memory_repo.clone(),
        infra.vision_llm_provider.clone(),
    ));

    let memory_learner = Arc::new(MemoryLearner::new(
        infra.llm_provider.clone(),
        infra.embedding_provider.clone(),
        repos.memory_repo.clone(),
        config_arc.clone(),
    ));

    let goal_learner = Arc::new(GoalLearner::new(
        infra.llm_provider.clone(),
        infra.embedding_provider.clone(),
        goals_service.clone(),
        config_arc.clone(),
    ));

    let memory_consolidator = Arc::new(MemoryConsolidator::new(
        infra.llm_provider.clone(),
        infra.embedding_provider.clone(),
        repos.memory_repo.clone(),
        config_arc.clone(),
    ));

    // ── Decision + generation ──────────────────────────────────────────
    let decision_engine = Arc::new(DecisionEngine::new(
        infra.llm_provider.clone(),
        repos.observation_repo.clone(),
        conversation_service.clone(),
        config_arc.clone(),
        Some(context_assembler.clone()),
    ));

    let tool_executor = Arc::new(ToolExecutor::new(
        tool_registry.clone(),
        config.tools.timeout_seconds,
    ));
    let tool_preselector = Arc::new(ToolPreselector::new(
        infra.llm_provider.clone(),
        config_arc.clone(),
    ));
    let tool_call_loop = Arc::new(ToolCallLoop::new(
        infra.llm_provider.clone(),
        tool_executor,
        config_arc.clone(),
    ));

    let proactive_generator = Arc::new(ProactiveGenerator::new(
        infra.llm_provider.clone(),
        context_assembler.clone(),
        conversation_service.clone(),
        infra.event_queue.clone(),
        config_arc.clone(),
        Some(repos.cooldown_repo.clone()),
        Some(tool_registry.clone()),
        Some(tool_call_loop.clone()),
    ));

    // ── Triggers ───────────────────────────────────────────────────────
    let screen_capture = Arc::new(ScreenCapture::new());

    let capture_trigger = CaptureTrigger::new(
        screen_capture.clone(),
        capture_learner,
        decision_engine.clone(),
        proactive_generator.clone(),
        Some(repos.cooldown_repo.clone()),
        repos.observation_repo.clone(),
        infra.event_queue.clone(),
        config_arc.clone(),
    );

    let checkin_trigger = CheckinTrigger::new(
        CheckinScheduler::new(
            config.checkin_times_vec(),
            config.checkin.interval_minutes,
            config.checkin.jitter_minutes,
            config.checkin.enabled,
        ),
        proactive_generator.clone(),
        conversation_service.clone(),
        Some(repos.cooldown_repo.clone()),
        config_arc.clone(),
    );

    let goal_trigger = Arc::new(GoalTrigger::new(
        repos.goal_repo.clone(),
        decision_engine,
        proactive_generator.clone(),
        Some(repos.cooldown_repo.clone()),
        infra.event_queue.clone(),
        config_arc.clone(),
    ));

    let agent_job_trigger = agent_job_manager.as_ref().map(|mgr| {
        Arc::new(AgentJobTrigger::new(
            mgr.clone(),
            repos.agent_job_repo.clone(),
            proactive_generator.clone(),
            config_arc.clone(),
            Some(infra.llm_provider.clone()),
        ))
    });

    // ── Runtime session ────────────────────────────────────────────────
    let runtime_session = Arc::new(RuntimeSession::new(
        checkin_trigger,
        goal_trigger,
        capture_trigger,
        Arc::new(MessageHandler::new(
            infra.llm_provider.clone(),
            context_assembler.clone(),
            conversation_service.clone(),
            message_learner,
            Some(repos.cooldown_repo.clone()),
            infra.event_queue.clone(),
            config_arc.clone(),
            Some(tool_registry.clone()),
            Some(tool_preselector),
            Some(tool_call_loop),
        )),
        conversation_service.clone(),
        Some(repos.cooldown_repo.clone()),
        infra.event_queue.clone(),
        config_arc.clone(),
        agent_job_trigger.clone(),
    ));

    // ── Learning loop (optional) ───────────────────────────────────────
    let learning_loop = config.learning.enabled.then(|| {
        Arc::new(LearningLoop::new(
            conversation_service.clone(),
            goals_service.clone(),
            memory_learner,
            goal_learner,
            memory_consolidator,
            repos.memory_repo.clone(),
            repos.observation_repo.clone(),
            repos.goal_repo.clone(),
            repos.learning_state_repo.clone(),
            infra.embedding_provider.clone(),
            config_arc.clone(),
        ))
    });

    // ── Goal worker ────────────────────────────────────────────────────
    let goal_worker = Arc::new(GoalWorker::new(
        config_arc.clone(),
        Arc::new(ClaudeAgentProvider::new(
            config_arc.clone(),
            infra.http_client.clone(),
        )),
        Arc::new(DefaultGoalContextProvider::new(
            repos.memory_repo.clone(),
            repos.goal_repo.clone(),
            repos.soul_repo.clone(),
            infra.embedding_provider.clone(),
        )),
        repos.goal_repo.clone(),
        repos.goal_plan_repo.clone(),
        infra.event_queue.clone(),
        conversation_service.clone(),
    ));

    let goal_worker_manager = GoalWorkerManager::new(
        config_arc.clone(),
        goal_worker,
        repos.goal_repo.clone(),
        repos.goal_plan_repo.clone(),
    );

    // ── Config manager ─────────────────────────────────────────────────
    let config_manager = Arc::new(ConfigManager::new(
        config_arc.clone(),
        infra.llm_swap_handle.clone(),
        infra.embedding_swap_handle.clone(),
        Some(infra.llm_factory.clone()),
    ));

    Wired {
        conversation_service,
        context_assembler,
        goals_service,
        tool_registry,
        runtime_session,
        learning_loop,
        screen_capture,
        config_manager,
        goal_worker_manager,
        mcp_adapter,
        native_adapter,
        agent_job_trigger,
    }
}

// ── Native tool construction ───────────────────────────────────────────────

fn build_native_tools(
    repos: &Repositories,
    embed: &Arc<dyn crate::llm::EmbeddingProvider>,
    agent_mgr: Option<&Arc<AgentJobManager>>,
) -> Vec<Arc<dyn NativeTool>> {
    use crate::tools::native::{
        approve_plan, archive_goal, browser_history, cancel_coding_agent, check_coding_agent,
        complete_goal, create_goal, create_memory, discover_git_repos, discover_installed_tools,
        fetch_url, file_reader, get_goals, get_recent_context, get_souls, launch_coding_agent,
        list_coding_agents, list_directory, pause_goal, reject_plan, resume_goal, search_context,
        search_files, search_goal, search_memories, update_goal, update_memory,
    };

    vec![
        Arc::new(search_memories::SearchMemoriesTool::new(
            repos.memory_repo.clone(),
            embed.clone(),
        )),
        Arc::new(search_context::SearchContextTool::new(
            repos.memory_repo.clone(),
            embed.clone(),
        )),
        Arc::new(search_goal::SearchGoalTool::new(
            repos.goal_repo.clone(),
            embed.clone(),
        )),
        Arc::new(get_goals::GetGoalsTool::new(repos.goal_repo.clone())),
        Arc::new(get_souls::GetSoulsTool::new(repos.soul_repo.clone())),
        Arc::new(get_recent_context::GetRecentContextTool::new(
            repos.observation_repo.clone(),
        )),
        Arc::new(create_memory::CreateMemoryTool::new(
            repos.memory_repo.clone(),
            embed.clone(),
        )),
        Arc::new(update_memory::UpdateMemoryTool::new(
            repos.memory_repo.clone(),
        )),
        Arc::new(create_goal::CreateGoalTool::new(
            repos.goal_repo.clone(),
            embed.clone(),
        )),
        Arc::new(update_goal::UpdateGoalTool::new(repos.goal_repo.clone())),
        Arc::new(complete_goal::CompleteGoalTool::new(
            repos.goal_repo.clone(),
        )),
        Arc::new(archive_goal::ArchiveGoalTool::new(repos.goal_repo.clone())),
        Arc::new(pause_goal::PauseGoalTool::new(repos.goal_repo.clone())),
        Arc::new(resume_goal::ResumeGoalTool::new(repos.goal_repo.clone())),
        Arc::new(approve_plan::ApprovePlanTool::new(
            repos.goal_plan_repo.clone(),
        )),
        Arc::new(reject_plan::RejectPlanTool::new(
            repos.goal_plan_repo.clone(),
        )),
        Arc::new(file_reader::FileReaderTool::new()),
        Arc::new(list_directory::ListDirectoryTool::new()),
        Arc::new(search_files::SearchFilesTool::new()),
        Arc::new(fetch_url::FetchUrlTool::new()),
        Arc::new(browser_history::BrowserHistoryTool::new()),
        Arc::new(discover_git_repos::DiscoverGitReposTool::new()),
        Arc::new(discover_installed_tools::DiscoverInstalledToolsTool::new()),
        Arc::new(launch_coding_agent::LaunchCodingAgentTool::new(
            agent_mgr.cloned(),
        )),
        Arc::new(check_coding_agent::CheckCodingAgentTool::new(
            repos.agent_job_repo.clone(),
        )),
        Arc::new(cancel_coding_agent::CancelCodingAgentTool::new(
            repos.agent_job_repo.clone(),
        )),
        Arc::new(list_coding_agents::ListCodingAgentsTool::new(
            repos.agent_job_repo.clone(),
        )),
    ]
}
