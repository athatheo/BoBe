//! `BobeHandler` — `SessionHandler` impl for BoBe sessions. Combines:
//!
//! * **Permission auto-approval** (sandbox-as-feature: workers run inside
//!   `~/.bobe/`).
//! * **Usage observation** — feed `assistant.usage` events into the
//!   shared `UsageMeter`.
//! * **Error surfacing** — `session.error` events go to the tracing log
//!   (the worker's `send_and_wait` future returns the error too, but
//!   the structured event has more fields).
//!
//! Per-event method overrides are preferred over a single `on_event`
//! match — keeps the dispatch shape obvious in stack traces.

use std::sync::Arc;

use async_trait::async_trait;
use github_copilot_sdk::handler::{PermissionResult, SessionHandler};
use github_copilot_sdk::types::{PermissionRequestData, RequestId, SessionEvent, SessionId};

use super::types::WorkerClass;
use super::usage::UsageMeter;

pub(crate) struct BobeHandler {
    class: WorkerClass,
    usage: Arc<UsageMeter>,
}

impl BobeHandler {
    pub(crate) fn new(class: WorkerClass, usage: Arc<UsageMeter>) -> Arc<Self> {
        Arc::new(Self { class, usage })
    }
}

#[async_trait]
impl SessionHandler for BobeHandler {
    async fn on_permission_request(
        &self,
        _session_id: SessionId,
        _request_id: RequestId,
        _data: PermissionRequestData,
    ) -> PermissionResult {
        // Workers run inside ~/.bobe/ — by design they can do anything in
        // there. Centralized so future tightening (e.g. block network
        // tools for batch workers) lands in one place.
        PermissionResult::Approved
    }

    async fn on_session_event(&self, _session_id: SessionId, event: SessionEvent) {
        match event.event_type.as_str() {
            "assistant.usage" => {
                self.usage.record(self.class, &event.data);
            }
            "session.error" => {
                let error_type = event
                    .data
                    .get("errorType")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let message = event
                    .data
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                tracing::warn!(
                    class = %self.class.name(),
                    error_type,
                    message,
                    "copilot session error"
                );
            }
            _ => {}
        }
    }
}
