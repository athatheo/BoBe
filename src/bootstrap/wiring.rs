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

pub(crate) struct Wired {
    pub(crate) conversation_service: Arc<ConversationService>,
    pub(crate) context_assembler: Arc<ContextAssembler>,
    pub(crate) goals_service: Arc<GoalsService>,
    pub(crate) tool_registry: Arc<ToolRegistry>,
    pub(crate) runtime_session: Arc<RuntimeSession>,
    pub(crate) learning_loop: Option<Arc<LearningLoop>>,
    pub(crate) screen_capture: Arc<ScreenCapture>,
    pub(crate) config_manager: Arc<ConfigManager>,
    pub(crate) goal_worker_manager: GoalWorkerManager,
    pub(crate) mcp_adapter: Arc<McpToolAdapter>,

    native_adapter: Arc<NativeToolAdapter>,
    agent_job_trigger: Option<Arc<AgentJobTrigger>>,
}

impl Wired {
    pub(crate) async fn register_tools(&self, config: &Config, _event_queue: &Arc<EventQueue>) {
        self.tool_registry
            .register(Arc::clone(&self.native_adapter) as Arc<dyn ToolSource>)
            .await;
        info!(
            tools = self.native_adapter.tool_names().len(),
            "bootstrap.native_tools_registered"
        );

        if config.mcp.enabled {
            match self.mcp_adapter.initialize().await {
                Ok(()) => {
                    self.tool_registry
                        .register(Arc::clone(&self.mcp_adapter) as Arc<dyn ToolSource>)
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

    pub(crate) async fn wire_sse_callbacks(&self, cm: &Arc<SseConnectionManager>) {
        let on_connect = {
            let rs = Arc::clone(&self.runtime_session);
            Box::new(move || {
                let rs = Arc::clone(&rs);
                tokio::spawn(async move { rs.on_connection().await });
            })
        };
        let on_disconnect = {
            let rs = Arc::clone(&self.runtime_session);
            Box::new(move || {
                let rs = Arc::clone(&rs);
                tokio::spawn(async move { rs.on_disconnection().await });
            })
        };
        cm.set_callbacks(on_connect, on_disconnect).await;
        info!("bootstrap.sse_callbacks_wired");
    }
}

// ── Assembly ───────────────────────────────────────────────────────────────

pub(crate) async fn wire(config: &Config, infra: &Infrastructure, repos: &Repositories) -> Wired {
    let config_arc = &infra.config_arc;

    let conversation_service = Arc::new(ConversationService::new(Arc::clone(
        &repos.conversation_repo,
    )));

    let soul_service = Arc::new(SoulService::new(
        config.soul_file.as_ref().map(PathBuf::from),
        Some(Arc::clone(&repos.soul_repo)),
    ));

    let context_assembler = Arc::new(ContextAssembler::new(
        Arc::clone(&repos.soul_repo),
        Arc::clone(&repos.goal_repo),
        Arc::clone(&repos.memory_repo),
        Arc::clone(&repos.observation_repo),
        Arc::clone(&repos.user_profile_repo),
        Arc::clone(&infra.embedding_provider),
        Some(soul_service),
    ));

    let goals_service = Arc::new(GoalsService::new(
        Arc::clone(&repos.goal_repo),
        Arc::clone(&infra.embedding_provider),
        Arc::clone(config_arc),
    ));

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
            Arc::clone(&repos.agent_job_repo),
            profiles,
            PathBuf::from(&config.coding_agent.output_dir),
            config.coding_agent.max_concurrent as usize, // safe: u32→usize on 64-bit
            config.coding_agent.max_runtime_seconds,
        ))
    });

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

    let message_learner = Arc::new(MessageLearner::new(
        Arc::clone(&infra.embedding_provider),
        Arc::clone(&repos.observation_repo),
    ));

    let capture_learner = Arc::new(CaptureLearner::new(
        Arc::clone(&infra.llm_provider),
        Arc::clone(&infra.embedding_provider),
        Arc::clone(&repos.observation_repo),
        Arc::clone(&repos.memory_repo),
        infra.vision_llm_provider.clone(),
        Arc::clone(config_arc),
    ));

    let memory_learner = Arc::new(MemoryLearner::new(
        Arc::clone(&infra.llm_provider),
        Arc::clone(&infra.embedding_provider),
        Arc::clone(&repos.memory_repo),
        Arc::clone(config_arc),
    ));

    let goal_learner = Arc::new(GoalLearner::new(
        Arc::clone(&infra.llm_provider),
        Arc::clone(&infra.embedding_provider),
        Arc::clone(&goals_service),
        Arc::clone(config_arc),
    ));

    let memory_consolidator = Arc::new(MemoryConsolidator::new(
        Arc::clone(&infra.llm_provider),
        Arc::clone(&infra.embedding_provider),
        Arc::clone(&repos.memory_repo),
        Arc::clone(config_arc),
    ));

    let decision_engine = Arc::new(DecisionEngine::new(
        Arc::clone(&infra.llm_provider),
        Arc::clone(&repos.observation_repo),
        Arc::clone(&conversation_service),
        Arc::clone(config_arc),
        Some(Arc::clone(&context_assembler)),
    ));

    let tool_executor = Arc::new(ToolExecutor::new(
        Arc::clone(&tool_registry),
        config.tools.timeout_seconds,
    ));
    let tool_preselector = Arc::new(ToolPreselector::new(
        Arc::clone(&infra.llm_provider),
        Arc::clone(config_arc),
    ));
    let tool_call_loop = Arc::new(ToolCallLoop::new(
        Arc::clone(&infra.llm_provider),
        tool_executor,
        Arc::clone(config_arc),
    ));

    let proactive_generator = Arc::new(ProactiveGenerator::new(
        Arc::clone(&infra.llm_provider),
        Arc::clone(&context_assembler),
        Arc::clone(&conversation_service),
        Arc::clone(&infra.event_queue),
        Arc::clone(config_arc),
        Some(Arc::clone(&repos.cooldown_repo)),
        Some(Arc::clone(&tool_registry)),
        Some(Arc::clone(&tool_call_loop)),
    ));

    let screen_capture = Arc::new(ScreenCapture::new());

    let capture_trigger = CaptureTrigger::new(
        Arc::clone(&screen_capture),
        capture_learner,
        Arc::clone(&decision_engine),
        Arc::clone(&proactive_generator),
        Some(Arc::clone(&repos.cooldown_repo)),
        Arc::clone(&repos.observation_repo),
        Arc::clone(&infra.event_queue),
        Arc::clone(config_arc),
    );

    let checkin_trigger = CheckinTrigger::new(
        CheckinScheduler::new(
            config.checkin_times_vec(),
            config.checkin.interval_minutes,
            config.checkin.jitter_minutes,
            config.checkin.enabled,
        ),
        Arc::clone(&proactive_generator),
        Arc::clone(&conversation_service),
        Some(Arc::clone(&repos.cooldown_repo)),
        Arc::clone(config_arc),
    );

    let goal_trigger = Arc::new(GoalTrigger::new(
        Arc::clone(&repos.goal_repo),
        decision_engine,
        Arc::clone(&proactive_generator),
        Some(Arc::clone(&repos.cooldown_repo)),
        Arc::clone(&infra.event_queue),
        Arc::clone(config_arc),
    ));

    let agent_job_trigger = agent_job_manager.as_ref().map(|mgr| {
        Arc::new(AgentJobTrigger::new(
            Arc::clone(mgr),
            Arc::clone(&repos.agent_job_repo),
            Arc::clone(&proactive_generator),
            Arc::clone(config_arc),
            Some(Arc::clone(&infra.llm_provider)),
        ))
    });

    let runtime_session = Arc::new(RuntimeSession::new(
        checkin_trigger,
        goal_trigger,
        capture_trigger,
        Arc::new(MessageHandler::new(
            Arc::clone(&infra.llm_provider),
            Arc::clone(&context_assembler),
            Arc::clone(&conversation_service),
            message_learner,
            Some(Arc::clone(&repos.cooldown_repo)),
            Arc::clone(&infra.event_queue),
            Arc::clone(config_arc),
            Some(Arc::clone(&tool_registry)),
            Some(tool_preselector),
            Some(tool_call_loop),
        )),
        Arc::clone(&conversation_service),
        Some(Arc::clone(&repos.cooldown_repo)),
        Arc::clone(&infra.event_queue),
        Arc::clone(config_arc),
        agent_job_trigger.clone(),
    ));

    let learning_loop = config.learning.enabled.then(|| {
        Arc::new(LearningLoop::new(
            Arc::clone(&conversation_service),
            Arc::clone(&goals_service),
            memory_learner,
            goal_learner,
            memory_consolidator,
            Arc::clone(&repos.memory_repo),
            Arc::clone(&repos.observation_repo),
            Arc::clone(&repos.goal_repo),
            Arc::clone(&repos.learning_state_repo),
            Arc::clone(&infra.embedding_provider),
            Arc::clone(config_arc),
        ))
    });

    let goal_worker = Arc::new(GoalWorker::new(
        Arc::clone(config_arc),
        Arc::new(ClaudeAgentProvider::new(
            Arc::clone(config_arc),
            infra.http_client.clone(),
        )),
        Arc::new(DefaultGoalContextProvider::new(
            Arc::clone(&repos.memory_repo),
            Arc::clone(&repos.goal_repo),
            Arc::clone(&repos.soul_repo),
            Arc::clone(&infra.embedding_provider),
        )),
        Arc::clone(&repos.goal_repo),
        Arc::clone(&repos.goal_plan_repo),
        Arc::clone(&infra.event_queue),
        Arc::clone(&conversation_service),
    ));

    let goal_worker_manager = GoalWorkerManager::new(
        Arc::clone(config_arc),
        goal_worker,
        Arc::clone(&repos.goal_repo),
        Arc::clone(&repos.goal_plan_repo),
    );

    let config_manager = Arc::new(ConfigManager::new(
        Arc::clone(config_arc),
        Arc::clone(&infra.llm_swap_handle),
        Arc::clone(&infra.embedding_swap_handle),
        Some(Arc::clone(&infra.llm_factory)),
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
            Arc::clone(&repos.memory_repo),
            Arc::clone(embed),
        )),
        Arc::new(search_context::SearchContextTool::new(
            Arc::clone(&repos.memory_repo),
            Arc::clone(embed),
        )),
        Arc::new(search_goal::SearchGoalTool::new(
            Arc::clone(&repos.goal_repo),
            Arc::clone(embed),
        )),
        Arc::new(get_goals::GetGoalsTool::new(Arc::clone(&repos.goal_repo))),
        Arc::new(get_souls::GetSoulsTool::new(Arc::clone(&repos.soul_repo))),
        Arc::new(get_recent_context::GetRecentContextTool::new(Arc::clone(
            &repos.observation_repo,
        ))),
        Arc::new(create_memory::CreateMemoryTool::new(
            Arc::clone(&repos.memory_repo),
            Arc::clone(embed),
        )),
        Arc::new(update_memory::UpdateMemoryTool::new(Arc::clone(
            &repos.memory_repo,
        ))),
        Arc::new(create_goal::CreateGoalTool::new(
            Arc::clone(&repos.goal_repo),
            Arc::clone(embed),
        )),
        Arc::new(update_goal::UpdateGoalTool::new(Arc::clone(
            &repos.goal_repo,
        ))),
        Arc::new(complete_goal::CompleteGoalTool::new(Arc::clone(
            &repos.goal_repo,
        ))),
        Arc::new(archive_goal::ArchiveGoalTool::new(Arc::clone(
            &repos.goal_repo,
        ))),
        Arc::new(pause_goal::PauseGoalTool::new(Arc::clone(&repos.goal_repo))),
        Arc::new(resume_goal::ResumeGoalTool::new(Arc::clone(
            &repos.goal_repo,
        ))),
        Arc::new(approve_plan::ApprovePlanTool::new(Arc::clone(
            &repos.goal_plan_repo,
        ))),
        Arc::new(reject_plan::RejectPlanTool::new(Arc::clone(
            &repos.goal_plan_repo,
        ))),
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
        Arc::new(check_coding_agent::CheckCodingAgentTool::new(Arc::clone(
            &repos.agent_job_repo,
        ))),
        Arc::new(cancel_coding_agent::CancelCodingAgentTool::new(Arc::clone(
            &repos.agent_job_repo,
        ))),
        Arc::new(list_coding_agents::ListCodingAgentsTool::new(Arc::clone(
            &repos.agent_job_repo,
        ))),
    ]
}
