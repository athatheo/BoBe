use std::sync::Arc;

use async_trait::async_trait;
use github_copilot_sdk::handler::{PermissionResult, SessionHandler};
use github_copilot_sdk::types::{PermissionRequestData, RequestId, SessionEvent, SessionId};

use super::types::WorkerClass;

pub(crate) struct BobeHandler {
    class: WorkerClass,
}

impl BobeHandler {
    pub(crate) fn new(class: WorkerClass) -> Arc<Self> {
        Arc::new(Self { class })
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
        // Workers sandboxed to ~/.bobe/; centralized for future per-class restrictions.
        PermissionResult::Approved
    }

    async fn on_session_event(&self, _session_id: SessionId, event: SessionEvent) {
        if event.event_type.as_str() == "session.error" {
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
    }
}
