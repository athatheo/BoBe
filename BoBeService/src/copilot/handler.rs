use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use github_copilot_sdk::handler::{PermissionHandler, PermissionResult};
use github_copilot_sdk::types::{
    PermissionRequestData, PermissionRequestKind, RequestId, SessionId,
};

use super::types::WorkerClass;

pub(crate) struct BobeHandler {
    class: WorkerClass,
    excluded_tools: HashSet<String>,
}

impl BobeHandler {
    pub(crate) fn new(class: WorkerClass, excluded_tools: &[String]) -> Arc<Self> {
        Arc::new(Self {
            class,
            excluded_tools: excluded_tools.iter().cloned().collect(),
        })
    }
}

#[async_trait]
impl PermissionHandler for BobeHandler {
    async fn handle(
        &self,
        _session_id: SessionId,
        _request_id: RequestId,
        data: PermissionRequestData,
    ) -> PermissionResult {
        if self.permission_allowed(&data) {
            tracing::debug!(
                class = %self.class.name(),
                ?data.kind,
                "copilot.permission_approved"
            );
            PermissionResult::approve_once()
        } else {
            tracing::warn!(
                class = %self.class.name(),
                ?data.kind,
                "copilot.permission_denied"
            );
            PermissionResult::reject(Some(
                "BoBe does not permit shell commands, file writes, or unknown permission types"
                    .to_string(),
            ))
        }
    }
}

impl BobeHandler {
    fn permission_allowed(&self, data: &PermissionRequestData) -> bool {
        if matches!(
            data.kind,
            Some(PermissionRequestKind::Mcp | PermissionRequestKind::CustomTool)
        ) && requested_tool_name(data).is_some_and(|tool| self.excluded_tools.contains(tool))
        {
            return false;
        }

        matches!(
            data.kind,
            Some(
                PermissionRequestKind::Read
                    | PermissionRequestKind::Url
                    | PermissionRequestKind::Mcp
                    | PermissionRequestKind::CustomTool
                    | PermissionRequestKind::Memory
                    | PermissionRequestKind::Hook
            )
        )
    }
}

fn requested_tool_name(data: &PermissionRequestData) -> Option<&str> {
    ["tool", "toolName", "name"]
        .into_iter()
        .find_map(|key| data.extra.get(key).and_then(serde_json::Value::as_str))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(kind: Option<PermissionRequestKind>, tool: Option<&str>) -> PermissionRequestData {
        PermissionRequestData {
            kind,
            extra: tool.map_or_else(
                || serde_json::json!({}),
                |tool| serde_json::json!({ "tool": tool }),
            ),
            ..PermissionRequestData::default()
        }
    }

    #[test]
    fn denies_mutating_and_unknown_permissions() {
        let handler = BobeHandler::new(WorkerClass::Chat, &[]);
        assert!(!handler.permission_allowed(&request(Some(PermissionRequestKind::Shell), None)));
        assert!(!handler.permission_allowed(&request(Some(PermissionRequestKind::Write), None)));
        assert!(!handler.permission_allowed(&request(Some(PermissionRequestKind::Unknown), None)));
        assert!(!handler.permission_allowed(&request(None, None)));
    }

    #[test]
    fn allows_read_only_and_explicit_integrations() {
        let handler = BobeHandler::new(WorkerClass::Chat, &[]);
        for kind in [
            PermissionRequestKind::Read,
            PermissionRequestKind::Mcp,
            PermissionRequestKind::CustomTool,
        ] {
            assert!(handler.permission_allowed(&request(Some(kind), None)));
        }
    }

    #[test]
    fn enforces_mcp_tool_exclusions() {
        let handler = BobeHandler::new(WorkerClass::Chat, &["delete_issue".to_string()]);
        assert!(!handler.permission_allowed(&request(
            Some(PermissionRequestKind::Mcp),
            Some("delete_issue")
        )));
        assert!(handler.permission_allowed(&request(
            Some(PermissionRequestKind::Mcp),
            Some("get_issue")
        )));
    }
}
