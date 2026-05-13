use std::sync::Arc;

use tracing::info;

use crate::config::Config;
use crate::config_manager::ConfigManager;
use crate::runtime::decision_engine::DecisionEngine;
use crate::runtime::learners::CaptureLearner;
use crate::runtime::message_handler::MessageHandler;
use crate::runtime::proactive_generator::ProactiveGenerator;
use crate::runtime::session::RuntimeSession;
use crate::runtime::triggers::capture_trigger::CaptureTrigger;
use crate::runtime::triggers::{CheckinScheduler, CheckinTrigger, GoalTrigger};
use crate::services::conversation_service::ConversationService;
use crate::services::goals::file_store::GoalFileStore;
use crate::services::goals::goals_service::GoalsService;
use crate::util::capture::ScreenCapture;
use crate::util::sse::connection_manager::SseConnectionManager;

use super::infra::Infrastructure;
use super::repos::Repositories;

pub(crate) struct Wired {
    pub(crate) goals_service: Arc<GoalsService>,
    pub(crate) runtime_session: Arc<RuntimeSession>,
    pub(crate) config_manager: Arc<ConfigManager>,
}

impl Wired {
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

    let goal_file_store = GoalFileStore::new(crate::util::paths::bobe_data_dir().join("goals"));
    let goals_service = Arc::new(GoalsService::new(Arc::clone(&goal_file_store)));

    let capture_learner = Arc::new(CaptureLearner::new(
        Arc::clone(&workers),
        workers.memory_file(),
    ));

    let decision_engine = Arc::new(DecisionEngine::new(
        Arc::clone(&workers),
        Arc::clone(&conversation_service),
        Arc::clone(config_arc),
    ));

    let proactive_generator = Arc::new(ProactiveGenerator::new(
        Arc::clone(&workers),
        Arc::clone(&conversation_service),
        Arc::clone(&infra.event_queue),
        Arc::clone(&repos.cooldown_repo),
    ));

    let screen_capture = Arc::new(ScreenCapture::new());

    let capture_trigger = CaptureTrigger::new(
        Arc::clone(&screen_capture),
        capture_learner,
        Arc::clone(&decision_engine),
        Arc::clone(&proactive_generator),
        Arc::clone(&repos.cooldown_repo),
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
        Arc::clone(&repos.cooldown_repo),
        Arc::clone(config_arc),
    );

    let goal_trigger = Arc::new(GoalTrigger::new(
        Arc::clone(&goals_service),
        decision_engine,
        Arc::clone(&proactive_generator),
        Arc::clone(&repos.cooldown_repo),
        Arc::clone(&infra.event_queue),
        Arc::clone(config_arc),
    ));

    let runtime_session = Arc::new(RuntimeSession::new(
        checkin_trigger,
        goal_trigger,
        capture_trigger,
        Arc::new(MessageHandler::new(
            Arc::clone(&workers),
            Arc::clone(&conversation_service),
            Arc::clone(&repos.cooldown_repo),
            Arc::clone(&infra.event_queue),
        )),
        Arc::clone(&conversation_service),
        Arc::clone(&repos.cooldown_repo),
        Arc::clone(&infra.event_queue),
        Arc::clone(config_arc),
    ));

    let config_manager = Arc::new(ConfigManager::new(Arc::clone(config_arc)));

    Wired {
        goals_service,
        runtime_session,
        config_manager,
    }
}
