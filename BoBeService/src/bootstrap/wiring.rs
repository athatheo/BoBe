use std::sync::Arc;

use tracing::info;

use crate::config::Config;
use crate::config::manager::ConfigManager;
use crate::runtime::behavior_context::BehaviorContext;
use crate::runtime::capture_learner::CaptureLearner;
use crate::runtime::conversation_service::ConversationService;
use crate::runtime::message_handler::MessageHandler;
use crate::runtime::proactive_generator::ProactiveGenerator;
use crate::runtime::session::RuntimeSession;
use crate::runtime::triggers::capture_trigger::CaptureTrigger;
use crate::runtime::triggers::{CheckinScheduler, CheckinTrigger, GoalTrigger};
use crate::services::goals::goals_service::GoalsService;
use crate::services::souls_service::SoulsService;
use crate::services::user_profile_service::UserProfileService;
use crate::util::capture::ScreenCapture;
use crate::util::sse::connection_manager::SseConnectionManager;

use super::infra::Infrastructure;
use super::repos::Repositories;

pub(crate) struct Wired {
    pub(crate) souls_service: Arc<SoulsService>,
    pub(crate) user_profile_service: Arc<UserProfileService>,
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
    goals_service: Arc<GoalsService>,
) -> Wired {
    let config_arc = &infra.config_arc;

    let conversation_service = Arc::new(ConversationService::new(Arc::clone(
        &repos.conversation_repo,
    )));

    let souls_service = Arc::new(SoulsService::new(Arc::clone(&repos.soul_repo)));
    let user_profile_service = Arc::new(UserProfileService::new(Arc::clone(
        &repos.user_profile_repo,
    )));

    let behavior_context = BehaviorContext::new(
        Arc::clone(&souls_service),
        Arc::clone(&user_profile_service),
    );

    let capture_learner = Arc::new(CaptureLearner::new(
        Arc::clone(&workers),
        workers.memory_file(),
    ));

    let proactive_generator = Arc::new(ProactiveGenerator::new(
        Arc::clone(&workers),
        Arc::clone(&conversation_service),
        Arc::clone(&infra.event_queue),
        Arc::clone(&repos.cooldown_repo),
        Arc::clone(&behavior_context),
        Arc::clone(config_arc),
    ));

    let screen_capture = Arc::new(ScreenCapture::new());

    let turn_admission = crate::runtime::turn_admission::TurnAdmission::new();

    let capture_trigger = CaptureTrigger::new(
        Arc::clone(&screen_capture),
        capture_learner,
        Arc::clone(&proactive_generator),
        Arc::clone(&repos.cooldown_repo),
        Arc::clone(&infra.event_queue),
        Arc::clone(config_arc),
        Arc::clone(&turn_admission),
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
        Arc::clone(&turn_admission),
    );

    let goal_trigger = Arc::new(GoalTrigger::new(
        Arc::clone(&goals_service),
        Arc::clone(&proactive_generator),
        Arc::clone(&repos.cooldown_repo),
        Arc::clone(&infra.event_queue),
        Arc::clone(config_arc),
        Arc::clone(&turn_admission),
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
            behavior_context,
        )),
        Arc::clone(&conversation_service),
        Arc::clone(&repos.cooldown_repo),
        Arc::clone(&infra.event_queue),
        Arc::clone(config_arc),
        turn_admission,
    ));

    let config_manager = Arc::new(ConfigManager::new(
        Arc::clone(config_arc),
        std::path::PathBuf::from(&config.data_dir),
    ));

    Wired {
        souls_service,
        user_profile_service,
        goals_service,
        runtime_session,
        config_manager,
    }
}
