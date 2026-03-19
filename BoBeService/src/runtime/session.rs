//! Top-level lifecycle manager for triggers, capture, and message handling.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use arc_swap::ArcSwap;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use crate::config::Config;
use crate::db::CooldownRepository;
use crate::runtime::message_handler::MessageHandler;
use crate::runtime::state::Decision;
use crate::runtime::triggers::agent_job_trigger::AgentJobTrigger;
use crate::runtime::triggers::capture_trigger::CaptureTrigger;
use crate::runtime::triggers::{CheckinTrigger, GoalTrigger};
use crate::services::conversation_service::ConversationService;
use crate::util::sse::event_queue::EventQueue;
use crate::util::sse::types::{EventType, IndicatorType, StreamBundle};

pub(crate) struct RuntimeSession {
    checkin_trigger: Mutex<CheckinTrigger>,
    goal_trigger: Arc<GoalTrigger>,
    capture_trigger: Mutex<CaptureTrigger>,
    message_handler: Arc<MessageHandler>,
    conversation: Arc<ConversationService>,
    cooldown_repo: Option<Arc<dyn CooldownRepository>>,
    event_queue: Arc<EventQueue>,
    config: Arc<ArcSwap<Config>>,
    agent_job_trigger: Option<Arc<AgentJobTrigger>>,
    running: std::sync::atomic::AtomicBool,
    capture_enabled: std::sync::atomic::AtomicBool,
    user_message_in_flight: Arc<AtomicBool>,
}

pub(crate) struct UserMessageGuard {
    user_message_in_flight: Arc<AtomicBool>,
}

impl Drop for UserMessageGuard {
    fn drop(&mut self) {
        self.user_message_in_flight.store(false, Ordering::Release);
    }
}

impl RuntimeSession {
    pub(crate) fn new(
        checkin_trigger: CheckinTrigger,
        goal_trigger: Arc<GoalTrigger>,
        capture_trigger: CaptureTrigger,
        message_handler: Arc<MessageHandler>,
        conversation: Arc<ConversationService>,
        cooldown_repo: Option<Arc<dyn CooldownRepository>>,
        event_queue: Arc<EventQueue>,
        config: Arc<ArcSwap<Config>>,
        agent_job_trigger: Option<Arc<AgentJobTrigger>>,
    ) -> Self {
        Self {
            checkin_trigger: Mutex::new(checkin_trigger),
            goal_trigger,
            capture_trigger: Mutex::new(capture_trigger),
            message_handler,
            conversation,
            cooldown_repo,
            event_queue,
            config,
            agent_job_trigger,
            running: std::sync::atomic::AtomicBool::new(false),
            capture_enabled: std::sync::atomic::AtomicBool::new(false),
            user_message_in_flight: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) async fn start_capture(&self) {
        self.capture_enabled
            .store(true, std::sync::atomic::Ordering::Release);
        let mut trigger = self.capture_trigger.lock().await;
        trigger.start().await;
        info!("runtime_session.capture_started");
    }

    pub(crate) async fn stop_capture(&self) {
        self.capture_enabled
            .store(false, std::sync::atomic::Ordering::Release);
        let mut trigger = self.capture_trigger.lock().await;
        trigger.stop().await;
        info!("runtime_session.capture_stopped");
    }

    pub(crate) async fn on_connection(&self) {
        let cfg = self.config.load();
        info!(
            capture_enabled = cfg.capture.enabled,
            "runtime_session.sse_client_connected"
        );
        if cfg.capture.enabled {
            self.start_capture().await;
        }
    }

    pub(crate) async fn on_disconnection(&self) {
        info!("runtime_session.sse_client_disconnected");
        self.stop_capture().await;
    }

    pub(crate) async fn start(&self) {
        self.running
            .store(true, std::sync::atomic::Ordering::Release);

        if let Some(ref cooldown_repo) = self.cooldown_repo
            && let Err(e) = cooldown_repo.load_or_create().await
        {
            warn!(error = %e, "runtime_session.cooldown_load_failed");
        }

        info!("runtime_session.started");
        self.event_queue.set_indicator(IndicatorType::Idle);
    }

    pub(crate) async fn stop(&self) {
        self.running
            .store(false, std::sync::atomic::Ordering::Release);
        info!("runtime_session.stopped");
    }

    pub(crate) async fn run(&self) {
        self.start().await;

        {
            let mut checkin = self.checkin_trigger.lock().await;
            let next = checkin.get_next_checkin_time();
            info!(
                next_checkin = ?next.map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string()),
                "runtime_session.scheduler_status"
            );
        }

        let mut loop_counter: u64 = 0;
        let maintenance_interval = std::time::Duration::from_mins(1);
        let mut last_goal_check = Instant::now();
        let mut last_capture_time = Instant::now();

        while self.running.load(std::sync::atomic::Ordering::Acquire) {
            loop_counter += 1;
            let cfg = self.config.load();

            if loop_counter.is_multiple_of(5) {
                self.log_heartbeat(loop_counter).await;
            }

            match tokio::time::timeout(std::time::Duration::from_mins(1), async {
                let mut checkin = self.checkin_trigger.lock().await;
                checkin.fire().await
            })
            .await
            {
                Ok(Decision::Engage) => {
                    info!(trigger = "checkin", "runtime_session.reach_out");
                }
                Ok(_) => {}
                Err(_) => {
                    warn!("runtime_session.checkin_trigger_timeout");
                }
            }

            if let Err(e) = self.close_stale_conversation_if_needed().await {
                warn!(error = %e, "runtime_session.stale_check_failed");
            }

            let time_since_goal = last_goal_check.elapsed().as_secs_f64();
            if time_since_goal >= cfg.goals.check_interval_seconds {
                match tokio::time::timeout(
                    std::time::Duration::from_secs(300),
                    self.goal_trigger.fire(),
                )
                .await
                {
                    Ok(Decision::Engage) => {
                        info!(trigger = "goal", "runtime_session.reach_out");
                    }
                    Ok(_) => {}
                    Err(_) => {
                        warn!("runtime_session.goal_trigger_timeout");
                    }
                }
                last_goal_check = Instant::now();
            }

            if self
                .capture_enabled
                .load(std::sync::atomic::Ordering::Acquire)
            {
                let time_since_capture = last_capture_time.elapsed().as_secs();
                if time_since_capture >= cfg.capture.interval_seconds {
                    match tokio::time::timeout(std::time::Duration::from_secs(300), async {
                        let mut ct = self.capture_trigger.lock().await;
                        ct.fire().await
                    })
                    .await
                    {
                        Ok(Decision::Engage) => {
                            info!(trigger = "capture", "runtime_session.reach_out");
                        }
                        Ok(_) => {}
                        Err(_) => {
                            warn!("runtime_session.capture_trigger_timeout");
                            self.push_error_event("capture_trigger", "Capture trigger timed out");
                        }
                    }
                    last_capture_time = Instant::now();
                }
            }

            if let Some(ref agent_trigger) = self.agent_job_trigger {
                match tokio::time::timeout(std::time::Duration::from_mins(1), agent_trigger.fire())
                    .await
                {
                    Ok(Decision::Engage) => {
                        info!(trigger = "agent_job", "runtime_session.reach_out");
                    }
                    Ok(_) => {}
                    Err(_) => {
                        warn!("runtime_session.agent_job_trigger_timeout");
                    }
                }
            }

            tokio::time::sleep(maintenance_interval).await;
        }

        self.stop().await;
    }

    async fn close_stale_conversation_if_needed(&self) -> Result<(), crate::error::AppError> {
        let cfg = self.config.load();
        let existing = self.conversation.get_pending_or_active().await?;
        let Some(existing) = existing else {
            return Ok(());
        };

        let turns = self
            .conversation
            .get_conversation_turns(existing.id, 100)
            .await?;
        if !existing.is_stale(cfg.conversation.auto_close_minutes as i64, &turns) {
            return Ok(());
        }

        if let Some(turn_count) = self
            .conversation
            .close_if_stale(existing.id, cfg.conversation.auto_close_minutes as i64, 100)
            .await?
        {
            info!(
                conversation_id = &existing.id.to_string()[..8],
                turn_count, "runtime_session.auto_closing_stale_conversation"
            );
        }
        Ok(())
    }

    async fn log_heartbeat(&self, loop_counter: u64) {
        let mut checkin = self.checkin_trigger.lock().await;
        let next_checkin = checkin.get_next_checkin_time();
        info!(
            loop_count = loop_counter,
            capture = self.capture_enabled.load(std::sync::atomic::Ordering::Acquire),
            next_checkin = ?next_checkin.map(|t| t.format("%H:%M:%S").to_string()),
            "runtime_session.heartbeat"
        );
    }

    pub(crate) async fn handle_user_message(&self, content: &str, message_id: &str) {
        self.message_handler
            .handle_message(content, message_id)
            .await;
    }

    pub(crate) fn try_begin_user_message(&self) -> Result<UserMessageGuard, &'static str> {
        if self
            .user_message_in_flight
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err("BoBe is still finishing the previous message");
        }

        let indicator = self.event_queue.current_indicator();
        if indicator != IndicatorType::Idle {
            self.user_message_in_flight.store(false, Ordering::Release);
            return Err(match indicator {
                IndicatorType::ScreenCapture => "BoBe is finishing capture work",
                IndicatorType::Thinking => "BoBe is still thinking",
                IndicatorType::ToolCalling => "BoBe is still using tools",
                IndicatorType::Streaming => "BoBe is still responding",
                IndicatorType::Idle => "BoBe is still finishing the previous message",
            });
        }

        Ok(UserMessageGuard {
            user_message_in_flight: Arc::clone(&self.user_message_in_flight),
        })
    }

    pub(crate) fn get_status(&self) -> serde_json::Value {
        serde_json::json!({
            "indicator": self.event_queue.current_indicator().as_str(),
            "capturing": self.capture_enabled.load(std::sync::atomic::Ordering::Acquire),
            "accepting_user_messages": self.event_queue.current_indicator() == IndicatorType::Idle
                && !self.user_message_in_flight.load(Ordering::Acquire),
        })
    }

    fn push_error_event(&self, trigger: &str, message: &str) {
        error!(trigger, message, "runtime_session.trigger_error");
        self.event_queue.push(StreamBundle {
            event_type: EventType::Error,
            message_id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            description: format!("{trigger} error"),
            payload: serde_json::json!({
                "trigger": trigger,
                "message": message,
                "recoverable": true,
            }),
        });
    }
}
