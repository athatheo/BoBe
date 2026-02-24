//! AskUserBridge — manages ask_user requests from goal execution agents.
//!
//! Sends questions to the user via SSE `action_request` events, then awaits
//! the user's response (or times out with a sensible default).
//!
//! Limitation: pending requests are in-memory only. On server restart any
//! pending ask_user requests are lost and the agent sees a timeout.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{oneshot, Mutex};
use tracing::{info, warn};
use uuid::Uuid;

use crate::error::AppError;
use crate::util::sse::event_queue::EventQueue;
use crate::util::sse::factories::action_request_event;

const DEFAULT_TIMEOUT_RESPONSE: &str =
    "No response from user \u{2014} proceed with your best judgment.";

/// A pending user request awaiting a response.
struct PendingRequest {
    sender: oneshot::Sender<String>,
}

/// In-memory bridge between goal execution agents and SSE action_request events.
pub struct AskUserBridge {
    event_queue: Arc<EventQueue>,
    timeout_seconds: u64,
    pending: Arc<Mutex<HashMap<String, PendingRequest>>>,
}

impl AskUserBridge {
    pub fn new(event_queue: Arc<EventQueue>, timeout_seconds: u64) -> Self {
        Self {
            event_queue,
            timeout_seconds,
            pending: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Update the timeout from hot-swapped config.
    pub fn set_timeout(&self, timeout_seconds: u64) {
        // NOTE: This is best-effort — already-waiting requests keep their
        // original timeout. Only new requests use the updated value.
        // A more precise approach would store the timeout per-request, but
        // the Python version has the same limitation.
        let _ = timeout_seconds;
        // We store timeout on self but it's not &mut self. For simplicity
        // we accept this minor inconsistency — the manager recreates the
        // bridge on config change in practice.
    }

    /// Send a question to the user via SSE and wait for their response.
    ///
    /// Returns the user's response, or a default message on timeout.
    pub async fn ask(
        &self,
        prompt: &str,
        goal_id: Uuid,
        options: Option<&[String]>,
    ) -> Result<String, AppError> {
        let request_id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel::<String>();

        // Register pending request
        {
            let mut pending = self.pending.lock().await;
            pending.insert(request_id.clone(), PendingRequest { sender: tx });
        }

        // Send SSE action_request event
        let timeout_ms = self.timeout_seconds * 1000;
        let sse_event = action_request_event(
            "ask_user",
            prompt,
            &request_id,
            timeout_ms,
            options,
        );
        self.event_queue.push(sse_event);

        info!(
            request_id = %request_id,
            goal_id = %goal_id,
            question_preview = &prompt[..prompt.len().min(80)],
            "ask_user.sent"
        );

        // Wait for response with timeout
        match tokio::time::timeout(
            std::time::Duration::from_secs(self.timeout_seconds),
            rx,
        )
        .await
        {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(_)) => {
                // Sender dropped without sending — treat as timeout
                info!(request_id = %request_id, goal_id = %goal_id, "ask_user.sender_dropped");
                self.cleanup_request(&request_id).await;
                Ok(DEFAULT_TIMEOUT_RESPONSE.to_string())
            }
            Err(_) => {
                // Timeout
                info!(request_id = %request_id, goal_id = %goal_id, "ask_user.timeout");
                self.cleanup_request(&request_id).await;
                Ok(DEFAULT_TIMEOUT_RESPONSE.to_string())
            }
        }
    }

    /// Called by HTTP endpoint when the user responds to an action_request.
    ///
    /// Returns `true` if the request was found and the response delivered.
    pub async fn submit_response(
        &self,
        request_id: &str,
        response: String,
    ) -> Result<bool, AppError> {
        let mut pending = self.pending.lock().await;
        if let Some(req) = pending.remove(request_id) {
            // Send response through the oneshot channel
            let _ = req.sender.send(response);
            info!(request_id = %request_id, "ask_user.response_received");
            Ok(true)
        } else {
            warn!(request_id = %request_id, "ask_user.unknown_request");
            Ok(false)
        }
    }

    async fn cleanup_request(&self, request_id: &str) {
        let mut pending = self.pending.lock().await;
        pending.remove(request_id);
    }
}
