use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use arc_swap::ArcSwap;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use crate::config::Config;
use crate::db::SqliteCooldownRepo;
use crate::runtime::conversation_service::ConversationService;
use crate::runtime::message_handler::MessageHandler;
use crate::runtime::state::Decision;
use crate::runtime::triggers::{CaptureTrigger, CheckinTrigger, GoalTrigger};
use crate::util::sse::event_queue::EventQueue;
use crate::util::sse::types::IndicatorType;

pub(crate) struct RuntimeSession {
    checkin_trigger: Mutex<CheckinTrigger>,
    goal_trigger: Arc<GoalTrigger>,
    capture_trigger: Mutex<CaptureTrigger>,
    message_handler: Arc<MessageHandler>,
    conversation: Arc<ConversationService>,
    cooldown_repo: Arc<SqliteCooldownRepo>,
    event_queue: Arc<EventQueue>,
    config: Arc<ArcSwap<Config>>,
    running: std::sync::atomic::AtomicBool,
    capture_enabled: std::sync::atomic::AtomicBool,
    user_message_in_flight: Arc<AtomicBool>,
}

/// RAII gate for "is a user-driven turn in flight." Cleared on drop so a
/// panicking turn doesn't brick the next message.
pub(crate) type UserMessageGuard = crate::util::atomic_flag_guard::AtomicFlagGuard;

#[derive(Debug, serde::Serialize)]
pub(crate) struct RuntimeStatus {
    pub(crate) indicator: IndicatorType,
    pub(crate) capturing: bool,
    pub(crate) accepting_user_messages: bool,
}

impl RuntimeSession {
    pub(crate) fn new(
        checkin_trigger: CheckinTrigger,
        goal_trigger: Arc<GoalTrigger>,
        capture_trigger: CaptureTrigger,
        message_handler: Arc<MessageHandler>,
        conversation: Arc<ConversationService>,
        cooldown_repo: Arc<SqliteCooldownRepo>,
        event_queue: Arc<EventQueue>,
        config: Arc<ArcSwap<Config>>,
        user_message_in_flight: Arc<AtomicBool>,
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
            running: std::sync::atomic::AtomicBool::new(false),
            capture_enabled: std::sync::atomic::AtomicBool::new(false),
            user_message_in_flight,
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

        if let Err(e) = self.cooldown_repo.load_or_create().await {
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
        let heartbeat_interval = std::time::Duration::from_mins(5);
        let mut last_heartbeat = Instant::now();
        let mut last_goal_check = Instant::now();
        let mut last_capture_time = Instant::now();

        while self.running.load(std::sync::atomic::Ordering::Acquire) {
            loop_counter += 1;
            let cfg = self.config.load();

            if last_heartbeat.elapsed() >= heartbeat_interval {
                self.log_heartbeat(loop_counter).await;
                last_heartbeat = Instant::now();
            }

            run_trigger("checkin", std::time::Duration::from_mins(1), async {
                let mut checkin = self.checkin_trigger.lock().await;
                checkin.fire().await
            })
            .await;

            if let Err(e) = self.close_stale_conversation_if_needed().await {
                warn!(error = %e, "runtime_session.stale_check_failed");
            }

            let time_since_goal = last_goal_check.elapsed().as_secs_f64();
            if time_since_goal >= cfg.goals.check_interval_seconds {
                run_trigger(
                    "goal",
                    std::time::Duration::from_mins(5),
                    self.goal_trigger.fire(),
                )
                .await;
                last_goal_check = Instant::now();
            }

            if self
                .capture_enabled
                .load(std::sync::atomic::Ordering::Acquire)
            {
                let time_since_capture = last_capture_time.elapsed().as_secs();
                if time_since_capture >= cfg.capture.interval_seconds {
                    let timed_out =
                        run_trigger("capture", std::time::Duration::from_mins(5), async {
                            let mut ct = self.capture_trigger.lock().await;
                            ct.fire().await
                        })
                        .await;
                    if timed_out {
                        self.push_error_event("capture_trigger", "Capture trigger timed out");
                    }
                    last_capture_time = Instant::now();
                }
            }

            // Sleep at the shortest configured cadence so a user-tuned
            // 5-second capture actually fires every 5 seconds. Floor at 1s
            // (avoid busy loop), ceiling at 60s (keep checkin scheduler
            // resolution under a minute).
            let goal_secs = cfg.goals.check_interval_seconds.max(1.0).round() as u64;
            let sleep_secs = cfg.capture.interval_seconds.max(1).min(goal_secs).min(60);
            tokio::time::sleep(std::time::Duration::from_secs(sleep_secs)).await;
        }

        self.stop().await;
    }

    async fn close_stale_conversation_if_needed(&self) -> Result<(), crate::error::AppError> {
        let cfg = self.config.load();
        let Some(existing) = self.conversation.get_pending_or_active().await? else {
            return Ok(());
        };

        // MAX(created_at) query instead of loading 100 turn rows just to
        // find the most recent user-turn timestamp.
        let last_user_at = self.conversation.last_user_turn_at(existing.id).await?;
        if !existing.is_stale_since(cfg.conversation.auto_close_minutes as i64, last_user_at) {
            return Ok(());
        }

        if let Some(turn_count) = self
            .conversation
            .close_if_stale(existing.id, cfg.conversation.auto_close_minutes as i64)
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

    /// Voice variant: text-delta observer runs in addition to SSE.
    /// `DeliveryMode::Immediate` makes barge-ins atomic without AbortGuard timing.
    pub(crate) async fn handle_user_message_with_observer<F>(
        &self,
        content: &str,
        message_id: &str,
        on_text_delta: F,
    ) where
        F: FnMut(&str) + Send,
    {
        self.message_handler
            .handle_message_with_observer(content, message_id, true, on_text_delta)
            .await;
    }

    pub(crate) fn try_begin_user_message(&self) -> Result<UserMessageGuard, &'static str> {
        let guard = UserMessageGuard::try_acquire(Arc::clone(&self.user_message_in_flight))
            .ok_or("BoBe is still finishing the previous message")?;

        let indicator = self.event_queue.current_indicator();
        if indicator != IndicatorType::Idle {
            // Drop releases the CAS we just made.
            drop(guard);
            return Err(match indicator {
                IndicatorType::ScreenCapture => "BoBe is finishing capture work",
                IndicatorType::Thinking => "BoBe is still thinking",
                IndicatorType::Streaming => "BoBe is still responding",
                IndicatorType::Idle => "BoBe is still finishing the previous message",
            });
        }

        Ok(guard)
    }

    pub(crate) fn get_status(&self) -> RuntimeStatus {
        let indicator = self.event_queue.current_indicator();
        RuntimeStatus {
            indicator,
            capturing: self.capture_enabled.load(Ordering::Acquire),
            accepting_user_messages: indicator == IndicatorType::Idle
                && !self.user_message_in_flight.load(Ordering::Acquire),
        }
    }

    fn push_error_event(&self, trigger: &str, message: &str) {
        error!(trigger, message, "runtime_session.trigger_error");
        self.event_queue
            .push(crate::util::sse::factories::trigger_error_event(
                trigger, message, true,
            ));
    }
}

/// Returns `true` if the timeout fired so the caller can surface an extra error event.
async fn run_trigger(
    name: &'static str,
    timeout: std::time::Duration,
    fut: impl std::future::Future<Output = Decision>,
) -> bool {
    match tokio::time::timeout(timeout, fut).await {
        Ok(Decision::Engage) => {
            info!(trigger = name, "runtime_session.reach_out");
            false
        }
        Ok(_) => false,
        Err(_) => {
            warn!(trigger = name, "runtime_session.trigger_timeout");
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Single-flight correctness rests on Drop clearing the flag on every exit.
    #[test]
    fn user_message_guard_clears_flag_on_drop() {
        let flag = Arc::new(AtomicBool::new(true));
        let guard = UserMessageGuard::new(Arc::clone(&flag));
        assert!(
            flag.load(Ordering::Acquire),
            "flag should be true while guard is alive"
        );
        drop(guard);
        assert!(
            !flag.load(Ordering::Acquire),
            "Drop must release the in-flight claim"
        );
    }

    /// Without unwind-safe Drop a single panicking turn would brick all future text messages.
    #[test]
    fn user_message_guard_clears_flag_on_panic() {
        let flag = Arc::new(AtomicBool::new(true));
        let flag_clone = Arc::clone(&flag);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _guard = UserMessageGuard::new(flag_clone);
            panic!("simulated turn failure");
        }));
        assert!(result.is_err(), "test setup: panic should propagate");
        assert!(
            !flag.load(Ordering::Acquire),
            "Drop fires during unwind — flag should be cleared even on panic"
        );
    }
}
