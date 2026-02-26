use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use arc_swap::ArcSwap;
use reqwest::Client;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tracing::{error, info, warn};

use crate::app_state::AppState;
use crate::config::{Config, LlmBackend};
use crate::config_manager::ConfigManager;
use crate::db::seeding as db_seeding;
use crate::db::{
    AgentJobRepository, ConversationRepository, CooldownRepository, GoalPlanRepository,
    GoalRepository, LearningStateRepository, McpConfigRepository, MemoryRepository,
    ObservationRepository, SoulRepository, UserProfileRepository,
};
use crate::db::{
    SqliteAgentJobRepo, SqliteConversationRepo, SqliteCooldownRepo, SqliteGoalPlanRepo,
    SqliteGoalRepo, SqliteLearningStateRepo, SqliteMcpConfigRepo, SqliteMemoryRepo,
    SqliteObservationRepo, SqliteSoulRepo, SqliteUserProfileRepo,
};
use crate::error::AppError;
use crate::llm::factory::LlmProviderFactory;
use crate::llm::ollama_manager::OllamaManager;
use crate::llm::swappable::{SwappableEmbeddingProvider, SwappableLlmProvider};
use crate::llm::{EmbeddingProvider, LlmProvider};
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
use crate::services::goals::goals_service::GoalsService;
use crate::services::soul_service::SoulService;
use crate::tools::executor::ToolExecutor;
use crate::tools::mcp::McpToolAdapter;
use crate::tools::native::adapter::NativeToolAdapter;
use crate::tools::native::base::NativeTool;
use crate::tools::preselector::ToolPreselector;
use crate::tools::registry::ToolRegistry;
use crate::tools::tool_call_loop::ToolCallLoop;
use crate::util::capture::ScreenCapture;
use crate::util::network::MdnsAnnouncer;
use crate::util::sse::connection_manager::SseConnectionManager;
use crate::util::sse::event_queue::EventQueue;

use crate::services::goal_worker::claude_provider::ClaudeAgentProvider;
use crate::services::goal_worker::context_provider::DefaultGoalContextProvider;
use crate::services::goal_worker::manager::GoalWorkerManager;
use crate::services::goal_worker::worker::GoalWorker;

/// Run the full application bootstrap sequence.
///
/// 1. Create SQLite pool
/// 2. Run migrations
/// 3. Wire all dependencies (repos, services, tools, runtime)
/// 4. Ensure Ollama is running (if backend is ollama)
/// 5. Seed default documents
/// 6. Build AppState
#[allow(clippy::too_many_lines)]
pub async fn run(config: Config) -> Result<(Arc<AppState>, GoalWorkerManager), AppError> {
    // ── Database setup ─────────────────────────────────────────────────
    let db_url = &config.database_url;
    if let Some(path) = db_url.strip_prefix("sqlite:")
        && let Some(parent) = std::path::Path::new(path).parent()
    {
        tokio::fs::create_dir_all(parent).await?;
    }

    let connect_options: SqliteConnectOptions = db_url
        .parse::<SqliteConnectOptions>()
        .map_err(AppError::Database)?
        .create_if_missing(true)
        .pragma("journal_mode", "WAL")
        .pragma("foreign_keys", "ON")
        .pragma("busy_timeout", "5000");

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(connect_options)
        .await
        .map_err(AppError::Database)?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|e| AppError::Database(e.into()))?;
    info!("database.migrations_complete");

    // ── HTTP client ────────────────────────────────────────────────────
    let config_arc = Arc::new(ArcSwap::from_pointee(config.clone()));
    let http_client = Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| AppError::Internal(format!("HTTP client build failed: {e}")))?;

    // ── LLM providers ──────────────────────────────────────────────────
    let llm_factory = Arc::new(LlmProviderFactory::new(
        http_client.clone(),
        config_arc.clone(),
    ));
    let real_provider = llm_factory.create(config.llm_backend)?;
    let vision_llm_provider = if config.vision_backend == LlmBackend::None {
        None
    } else {
        Some(llm_factory.create_vision(config.vision_backend)?)
    };

    // Wrap in SwappableLlmProvider so all consumers automatically see
    // the latest provider after a hot-swap (no update callbacks needed).
    let (swappable_provider, swap_handle) = SwappableLlmProvider::new(real_provider);
    let llm_provider: Arc<dyn LlmProvider> = Arc::new(swappable_provider);

    // ── Embedding ──────────────────────────────────────────────────────
    let real_embedding_provider = llm_factory.create_embedding()?;
    let (swappable_embedding_provider, embedding_swap_handle) =
        SwappableEmbeddingProvider::new(real_embedding_provider);
    let embedding_provider: Arc<dyn EmbeddingProvider> = Arc::new(swappable_embedding_provider);

    // ── SSE ────────────────────────────────────────────────────────────
    let event_queue = Arc::new(EventQueue::new(100));
    let connection_manager = Arc::new(SseConnectionManager::new(event_queue.clone(), None, None));

    // ── Repos ──────────────────────────────────────────────────────────
    let conversation_repo: Arc<dyn ConversationRepository> =
        Arc::new(SqliteConversationRepo::new(pool.clone()));
    let memory_repo: Arc<dyn MemoryRepository> = Arc::new(SqliteMemoryRepo::new(pool.clone()));
    let goal_repo: Arc<dyn GoalRepository> = Arc::new(SqliteGoalRepo::new(pool.clone()));
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
    let soul_repo: Arc<dyn SoulRepository> = Arc::new(SqliteSoulRepo::new(pool.clone()));
    let user_profile_repo: Arc<dyn UserProfileRepository> =
        Arc::new(SqliteUserProfileRepo::new(pool.clone()));
    let goal_plan_repo: Arc<dyn GoalPlanRepository> =
        Arc::new(SqliteGoalPlanRepo::new(pool.clone()));

    // ── Services ───────────────────────────────────────────────────────
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

    let goals_service = Arc::new(GoalsService::new(
        goal_repo.clone(),
        embedding_provider.clone(),
        config_arc.clone(),
    ));

    // ── Tool registry ──────────────────────────────────────────────────
    let tool_registry = Arc::new(ToolRegistry::new());
    let native_tools: Vec<Arc<dyn NativeTool>> = vec![
        Arc::new(
            crate::tools::native::search_memories::SearchMemoriesTool::new(
                memory_repo.clone(),
                embedding_provider.clone(),
            ),
        ),
        Arc::new(
            crate::tools::native::search_context::SearchContextTool::new(
                memory_repo.clone(),
                embedding_provider.clone(),
            ),
        ),
        Arc::new(crate::tools::native::search_goal::SearchGoalTool::new(
            goal_repo.clone(),
            embedding_provider.clone(),
        )),
        Arc::new(crate::tools::native::get_goals::GetGoalsTool::new(
            goal_repo.clone(),
        )),
        Arc::new(crate::tools::native::get_souls::GetSoulsTool::new(
            soul_repo.clone(),
        )),
        Arc::new(
            crate::tools::native::get_recent_context::GetRecentContextTool::new(
                observation_repo.clone(),
            ),
        ),
        Arc::new(crate::tools::native::create_memory::CreateMemoryTool::new(
            memory_repo.clone(),
            embedding_provider.clone(),
        )),
        Arc::new(crate::tools::native::update_memory::UpdateMemoryTool::new(
            memory_repo.clone(),
        )),
        Arc::new(crate::tools::native::create_goal::CreateGoalTool::new(
            goal_repo.clone(),
            embedding_provider.clone(),
        )),
        Arc::new(crate::tools::native::update_goal::UpdateGoalTool::new(
            goal_repo.clone(),
        )),
        Arc::new(crate::tools::native::complete_goal::CompleteGoalTool::new(
            goal_repo.clone(),
        )),
        Arc::new(crate::tools::native::archive_goal::ArchiveGoalTool::new(
            goal_repo.clone(),
        )),
        Arc::new(crate::tools::native::pause_goal::PauseGoalTool::new(
            goal_repo.clone(),
        )),
        Arc::new(crate::tools::native::resume_goal::ResumeGoalTool::new(
            goal_repo.clone(),
        )),
        Arc::new(crate::tools::native::approve_plan::ApprovePlanTool::new(
            goal_plan_repo.clone(),
        )),
        Arc::new(crate::tools::native::reject_plan::RejectPlanTool::new(
            goal_plan_repo.clone(),
        )),
        Arc::new(crate::tools::native::file_reader::FileReaderTool::new()),
        Arc::new(crate::tools::native::list_directory::ListDirectoryTool::new()),
        Arc::new(crate::tools::native::search_files::SearchFilesTool::new()),
        Arc::new(crate::tools::native::fetch_url::FetchUrlTool::new()),
        Arc::new(crate::tools::native::browser_history::BrowserHistoryTool::new()),
        Arc::new(crate::tools::native::discover_git_repos::DiscoverGitReposTool::new()),
        Arc::new(crate::tools::native::discover_installed_tools::DiscoverInstalledToolsTool::new()),
        Arc::new(
            crate::tools::native::launch_coding_agent::LaunchCodingAgentTool::new(
                agent_job_repo.clone(),
            ),
        ),
        Arc::new(
            crate::tools::native::check_coding_agent::CheckCodingAgentTool::new(
                agent_job_repo.clone(),
            ),
        ),
        Arc::new(
            crate::tools::native::cancel_coding_agent::CancelCodingAgentTool::new(
                agent_job_repo.clone(),
            ),
        ),
        Arc::new(
            crate::tools::native::list_coding_agents::ListCodingAgentsTool::new(
                agent_job_repo.clone(),
            ),
        ),
    ];
    let native_adapter = Arc::new(NativeToolAdapter::new(native_tools));
    let mcp_adapter = Arc::new(McpToolAdapter::new(
        if config.mcp_enabled {
            Some(mcp_config_repo.clone())
        } else {
            None
        },
        config.mcp_blocked_commands_vec(),
        config.mcp_dangerous_env_keys_vec(),
    ));

    // ── Learners ───────────────────────────────────────────────────────
    let message_learner = Arc::new(MessageLearner::new(
        embedding_provider.clone(),
        observation_repo.clone(),
    ));

    let capture_learner = Arc::new(CaptureLearner::new(
        llm_provider.clone(),
        embedding_provider.clone(),
        observation_repo.clone(),
        memory_repo.clone(),
        vision_llm_provider.clone(),
    ));

    let memory_learner = Arc::new(MemoryLearner::new(
        llm_provider.clone(),
        embedding_provider.clone(),
        memory_repo.clone(),
        config_arc.clone(),
    ));

    let goal_learner = Arc::new(GoalLearner::new(
        llm_provider.clone(),
        embedding_provider.clone(),
        goals_service.clone(),
        config_arc.clone(),
    ));

    let memory_consolidator = Arc::new(MemoryConsolidator::new(
        llm_provider.clone(),
        embedding_provider.clone(),
        memory_repo.clone(),
        config_arc.clone(),
    ));

    // ── Decision engine + proactive generator ──────────────────────────
    let decision_engine = Arc::new(DecisionEngine::new(
        llm_provider.clone(),
        observation_repo.clone(),
        conversation_service.clone(),
        config_arc.clone(),
        Some(context_assembler.clone()),
    ));

    // ── Tool Call Loop + Preselector ───────────────────────────────────
    let tool_executor = Arc::new(ToolExecutor::new(
        tool_registry.clone(),
        config.tools_timeout_seconds,
    ));
    let tool_preselector = Arc::new(ToolPreselector::new(
        llm_provider.clone(),
        config_arc.clone(),
    ));
    let tool_call_loop = Arc::new(ToolCallLoop::new(
        llm_provider.clone(),
        tool_executor,
        config_arc.clone(),
    ));

    let proactive_generator = Arc::new(ProactiveGenerator::new(
        llm_provider.clone(),
        context_assembler.clone(),
        conversation_service.clone(),
        event_queue.clone(),
        config_arc.clone(),
        Some(cooldown_repo.clone()),
        Some(tool_registry.clone()),
        Some(tool_call_loop.clone()),
    ));

    // ── Capture ────────────────────────────────────────────────────────
    let screen_capture = Arc::new(ScreenCapture::new());

    // ── Triggers ───────────────────────────────────────────────────────
    let capture_trigger = CaptureTrigger::new(
        screen_capture.clone(),
        capture_learner,
        decision_engine.clone(),
        proactive_generator.clone(),
        Some(cooldown_repo.clone()),
        observation_repo.clone(),
        event_queue.clone(),
        config_arc.clone(),
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
        config_arc.clone(),
    );

    let goal_trigger = Arc::new(GoalTrigger::new(
        goal_repo.clone(),
        decision_engine,
        proactive_generator.clone(),
        Some(cooldown_repo.clone()),
        event_queue.clone(),
        config_arc.clone(),
    ));

    // Agent job trigger (optional)
    let agent_job_trigger = if config.coding_agents_enabled {
        let profiles: HashMap<String, _> =
            serde_json::from_str(&config.coding_agent_profiles).unwrap_or_default();
        let agent_job_manager = Arc::new(AgentJobManager::new(
            agent_job_repo.clone(),
            profiles,
            PathBuf::from(&config.coding_agent_output_dir),
            config.coding_agent_max_concurrent as usize,
            config.coding_agent_max_runtime_seconds,
        ));
        let trigger = Arc::new(AgentJobTrigger::new(
            agent_job_manager,
            agent_job_repo.clone(),
            proactive_generator,
            config_arc.clone(),
            Some(llm_provider.clone()),
        ));
        Some(trigger)
    } else {
        None
    };

    // ── RuntimeSession ─────────────────────────────────────────────────
    let runtime_session = Arc::new(RuntimeSession::new(
        checkin_trigger,
        goal_trigger,
        capture_trigger,
        Arc::new(MessageHandler::new(
            llm_provider.clone(),
            context_assembler.clone(),
            conversation_service.clone(),
            message_learner,
            Some(cooldown_repo.clone()),
            event_queue.clone(),
            config_arc.clone(),
            Some(tool_registry.clone()),
            Some(tool_preselector),
            Some(tool_call_loop),
        )),
        conversation_service.clone(),
        Some(cooldown_repo.clone()),
        event_queue.clone(),
        config_arc.clone(),
        agent_job_trigger.clone(),
    ));

    // ── LearningLoop (optional) ────────────────────────────────────────
    let learning_loop = if config.learning_enabled {
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
            config_arc.clone(),
        )))
    } else {
        None
    };

    // ── Goal Worker ────────────────────────────────────────────────────
    let goal_context_provider = Arc::new(DefaultGoalContextProvider::new(
        memory_repo.clone(),
        goal_repo.clone(),
        soul_repo.clone(),
        embedding_provider.clone(),
    ));

    let claude_agent_provider = Arc::new(ClaudeAgentProvider::new(
        config_arc.clone(),
        http_client.clone(),
    ));

    let goal_worker = Arc::new(GoalWorker::new(
        config_arc.clone(),
        claude_agent_provider,
        goal_context_provider,
        goal_repo.clone(),
        goal_plan_repo.clone(),
        event_queue.clone(),
        conversation_service.clone(),
    ));

    let goal_worker_manager = GoalWorkerManager::new(
        config_arc.clone(),
        goal_worker,
        goal_repo.clone(),
        goal_plan_repo.clone(),
    );

    // ── Ollama manager ─────────────────────────────────────────────────
    let ollama_manager = Arc::new(OllamaManager::new(
        http_client.clone(),
        &config.ollama_url,
        &config.ollama_model,
        config.ollama_auto_start,
        config.ollama_auto_pull,
        config.ollama_binary_path.clone(),
    ));

    // ── Network ────────────────────────────────────────────────────────
    let mdns_announcer = Arc::new(MdnsAnnouncer::new(
        config.port,
        config.mdns_enabled && config.host == "0.0.0.0",
    ));

    // ── Config manager ─────────────────────────────────────────────────
    let config_manager = Arc::new(ConfigManager::new(
        config_arc.clone(),
        swap_handle,
        embedding_swap_handle,
        Some(llm_factory),
    ));

    // ── Post-build setup ───────────────────────────────────────────────

    // Ensure Ollama is running (if using Ollama backend)
    if config.llm_backend == LlmBackend::Ollama || config.vision_backend == LlmBackend::Ollama {
        match ollama_manager.ensure_running().await {
            Ok(()) => info!(model = %config.ollama_model, "ollama.ready"),
            Err(e) => {
                error!(error = %e, "ollama.startup_failed");
                // Don't fail hard — LLM might become available later
                warn!("Continuing without Ollama — LLM calls will fail until it's available");
            }
        }

        // Also ensure vision model if needed
        if config.vision_backend == LlmBackend::Ollama {
            match ollama_manager
                .ensure_model(&config.vision_ollama_model)
                .await
            {
                Ok(true) => info!(model = %config.vision_ollama_model, "ollama.vision_ready"),
                Ok(false) => {
                    warn!(model = %config.vision_ollama_model, "ollama.vision_model_unavailable")
                }
                Err(e) => warn!(error = %e, "ollama.vision_model_check_failed"),
            }
        }
    }

    // Seed default documents
    if config.seed_default_documents {
        if let Err(e) = db_seeding::seed_default_souls(soul_repo.as_ref()).await {
            warn!(error = %e, "bootstrap.soul_seeding_failed");
        }
        if let Err(e) = db_seeding::seed_default_user_profiles(user_profile_repo.as_ref()).await {
            warn!(error = %e, "bootstrap.user_profile_seeding_failed");
        }
    }

    // Start mDNS if enabled
    mdns_announcer.start().await;

    // Register native tools with the registry
    {
        tool_registry
            .register(native_adapter.clone() as Arc<dyn crate::tools::ToolSource>)
            .await;
        info!(
            tools = native_adapter.tool_names().len(),
            "bootstrap.native_tools_registered"
        );
    }

    // Register MCP tools if enabled
    if config.mcp_enabled {
        match mcp_adapter.initialize().await {
            Ok(()) => {
                tool_registry
                    .register(mcp_adapter.clone() as Arc<dyn crate::tools::ToolSource>)
                    .await;
                info!("bootstrap.mcp_tools_registered");
            }
            Err(e) => {
                warn!(error = %e, "bootstrap.mcp_initialization_failed");
            }
        }
    }

    // Mark orphaned agent jobs as failed
    {
        use crate::models::types::AgentJobStatus;
        match agent_job_repo.find_by_status(AgentJobStatus::Running).await {
            Ok(orphans) if !orphans.is_empty() => {
                info!(count = orphans.len(), "bootstrap.orphaned_jobs_found");
                for mut job in orphans {
                    job.mark_failed("Orphaned on restart".to_string(), None);
                    if let Err(e) = agent_job_repo.save(&job).await {
                        warn!(job_id = %job.id, error = %e, "bootstrap.orphan_mark_failed");
                    }
                }
            }
            Ok(_) => {}
            Err(e) => warn!(error = %e, "bootstrap.orphan_check_failed"),
        }
    }

    // Clean corrupt embeddings (NULL out non-JSON-array values)
    {
        let tables = ["memories", "observations"];
        let mut total_cleaned: u64 = 0;
        for table in tables {
            let sql = format!(
                "UPDATE {} SET embedding = NULL WHERE embedding IS NOT NULL AND embedding NOT LIKE '[%'",
                table
            );
            match sqlx::query(&sql).execute(&pool).await {
                Ok(result) => {
                    let rows = result.rows_affected();
                    if rows > 0 {
                        warn!(table, rows, "bootstrap.cleaned_corrupt_embeddings");
                        total_cleaned += rows;
                    }
                }
                Err(e) => warn!(error = %e, table, "bootstrap.embedding_cleanup_failed"),
            }
        }
        if total_cleaned > 0 {
            info!(
                total_rows = total_cleaned,
                "bootstrap.corrupt_embeddings_cleaned"
            );
        }
    }

    // Sync goals from file if configured
    if config.goals_sync_on_startup {
        match goals_service.sync_from_file().await {
            Ok(result) => {
                info!(
                    created = result.created,
                    updated = result.updated,
                    archived = result.archived,
                    "bootstrap.goals_synced_from_file"
                );
            }
            Err(e) => warn!(error = %e, "bootstrap.goals_sync_failed"),
        }
    }

    // Wire SSE callbacks to RuntimeSession (connect -> start capture, disconnect -> stop)
    {
        let rs = runtime_session.clone();
        let rs2 = runtime_session.clone();
        connection_manager
            .set_callbacks(
                Box::new(move || {
                    let rs = rs.clone();
                    tokio::spawn(async move { rs.on_connection().await });
                }),
                Box::new(move || {
                    let rs = rs2.clone();
                    tokio::spawn(async move { rs.on_disconnection().await });
                }),
            )
            .await;
        info!("bootstrap.sse_callbacks_wired");
    }

    // Register agent job trigger callback for immediate job completion handling
    if let Some(ref trigger) = agent_job_trigger {
        trigger.register_callback().await;
    }

    // ── Build AppState ─────────────────────────────────────────────────
    let state = Arc::new(AppState {
        db: pool,
        config: config_arc,
        http_client,
        event_queue,
        connection_manager,
        llm_provider,
        vision_llm_provider,
        embedding_provider,
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
        goal_plan_repo,
        conversation_service,
        context_assembler,
        goals_service,
        tool_registry,
        runtime_session,
        learning_loop,
        screen_capture,
        ollama_manager,
        config_manager,
        mcp_tool_adapter: Some(mcp_adapter),
        mdns_announcer,
    });

    // Print startup banner
    print_banner(&config);

    Ok((state, goal_worker_manager))
}

fn print_banner(config: &Config) {
    info!("═══════════════════════════════════════════════════════");
    info!("  BoBe Server Started");
    info!("  LLM backend: {}", config.llm_backend);
    info!("  Model: {}", config.ollama_model);
    info!("  Capture enabled: {}", config.capture_enabled);
    info!("  Learning enabled: {}", config.learning_enabled);
    info!("  Tools enabled: {}", config.tools_enabled);
    info!("═══════════════════════════════════════════════════════");
}
