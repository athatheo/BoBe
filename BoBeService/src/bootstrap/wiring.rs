//! Dependency wiring — connects services, tools, learners, triggers, and the
//! runtime session into a coherent application graph.
//!
//! This is the only module that knows all concrete types. Everything it
//! produces is behind `Arc<dyn Trait>` or a concrete `Arc<T>`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tracing::info;

use crate::config::Config;
use crate::config_manager::ConfigManager;
use crate::runtime::decision_engine::DecisionEngine;
use crate::runtime::learners::CaptureLearner;
use crate::runtime::message_handler::MessageHandler;
use crate::runtime::proactive_generator::ProactiveGenerator;
use crate::runtime::session::RuntimeSession;
use crate::runtime::triggers::agent_job_trigger::AgentJobTrigger;
use crate::runtime::triggers::capture_trigger::CaptureTrigger;
use crate::runtime::triggers::{CheckinScheduler, CheckinTrigger, GoalTrigger};
use crate::services::agent_job_manager::AgentJobManager;
use crate::services::conversation_service::ConversationService;
use crate::services::goals::file_store::GoalFileStore;
use crate::services::goals::goals_service::GoalsService;
use crate::util::capture::ScreenCapture;
use crate::util::sse::connection_manager::SseConnectionManager;

use super::infra::Infrastructure;
use super::repos::Repositories;

pub(crate) struct Wired {
    pub(crate) conversation_service: Arc<ConversationService>,
    pub(crate) goals_service: Arc<GoalsService>,
    pub(crate) runtime_session: Arc<RuntimeSession>,
    pub(crate) screen_capture: Arc<ScreenCapture>,
    pub(crate) config_manager: Arc<ConfigManager>,

    agent_job_trigger: Option<Arc<AgentJobTrigger>>,
}

impl Wired {
    pub(crate) async fn start_services(&self) {
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

pub(crate) async fn wire(
    config: &Config,
    infra: &Infrastructure,
    repos: &Repositories,
    workers: Arc<crate::copilot::registry::WorkerRegistry>,
) -> Wired {
    let config_arc = &infra.config_arc;

    let conversation_service = Arc::new(ConversationService::new(Arc::clone(
        &repos.conversation_repo,
    )));

    // File-backed goals: each goal is `~/.bobe/goals/<id>.md`. The
    // chat agent reads/edits these via SDK Read/Write/Edit; the API +
    // trigger go through `GoalsService` for the same dir.
    let goal_file_store = GoalFileStore::new(crate::util::paths::bobe_data_dir().join("goals"));
    let goals_service = Arc::new(GoalsService::new(Arc::clone(&goal_file_store)));

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

    // Memory.md is the single durable narrative store. The capture
    // learner appends one-liners under `## Recent`; the consolidate
    // worker prunes the section nightly.
    let capture_learner = Arc::new(CaptureLearner::new(
        Arc::clone(&workers),
        workers.memory_file(),
        Arc::clone(config_arc),
    ));

    let decision_engine = Arc::new(DecisionEngine::new(
        Arc::clone(&workers),
        Arc::clone(&conversation_service),
        Arc::clone(config_arc),
    ));

    // Tool dispatch is now owned by Copilot SDK's session loop —
    // built-in Read/Write/Bash/Grep tools auto-invoked in autopilot
    // mode. The legacy `ToolPreselector`, `ToolCallLoop`, and
    // `ToolExecutor` modules remain as deprecated reference (see
    // `#[deprecated]` on each) but are no longer constructed at boot.

    let proactive_generator = Arc::new(ProactiveGenerator::new(
        Arc::clone(&workers),
        Arc::clone(&conversation_service),
        Arc::clone(&infra.event_queue),
        Some(Arc::clone(&repos.cooldown_repo)),
    ));

    let screen_capture = Arc::new(ScreenCapture::new());

    let capture_trigger = CaptureTrigger::new(
        Arc::clone(&screen_capture),
        capture_learner,
        Arc::clone(&decision_engine),
        Arc::clone(&proactive_generator),
        Some(Arc::clone(&repos.cooldown_repo)),
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
        Arc::clone(&goals_service),
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
            Arc::clone(&workers),
        ))
    });

    let runtime_session = Arc::new(RuntimeSession::new(
        checkin_trigger,
        goal_trigger,
        capture_trigger,
        Arc::new(MessageHandler::new(
            Arc::clone(&workers),
            Arc::clone(&conversation_service),
            Some(Arc::clone(&repos.cooldown_repo)),
            Arc::clone(&infra.event_queue),
        )),
        Arc::clone(&conversation_service),
        Some(Arc::clone(&repos.cooldown_repo)),
        Arc::clone(&infra.event_queue),
        Arc::clone(config_arc),
        agent_job_trigger.clone(),
    ));

    let config_manager = Arc::new(ConfigManager::new(Arc::clone(config_arc)));

    Wired {
        conversation_service,
        goals_service,
        runtime_session,
        screen_capture,
        config_manager,
        agent_job_trigger,
    }
}
