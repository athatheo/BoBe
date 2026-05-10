//! Read + patch the live config.

use std::collections::HashMap;
use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::error::AppError;

#[derive(Debug, Serialize)]
pub(crate) struct SettingsResponse {
    pub(crate) capture_enabled: bool,
    pub(crate) capture_interval_seconds: u64,
    pub(crate) checkin_enabled: bool,
    pub(crate) checkin_times: Vec<String>,
    pub(crate) checkin_jitter_minutes: u32,
    pub(crate) conversation_inactivity_timeout_seconds: u64,
    pub(crate) conversation_auto_close_minutes: u64,
    pub(crate) goal_check_interval_seconds: f64,
    pub(crate) mcp_enabled: bool,
    /// "copilot_cloud" (default) or "local". Restart-required.
    pub(crate) engine: String,
    pub(crate) provider_base_url: Option<String>,
    pub(crate) provider_text_model: Option<String>,
    pub(crate) provider_vision_model: Option<String>,
    pub(crate) provider_offline: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SettingsUpdateRequest {
    pub(crate) capture_enabled: Option<bool>,
    pub(crate) capture_interval_seconds: Option<u64>,
    pub(crate) checkin_enabled: Option<bool>,
    pub(crate) checkin_times: Option<Vec<String>>,
    pub(crate) checkin_jitter_minutes: Option<u32>,
    pub(crate) conversation_inactivity_timeout_seconds: Option<u64>,
    pub(crate) conversation_auto_close_minutes: Option<u64>,
    pub(crate) goal_check_interval_seconds: Option<f64>,
    pub(crate) mcp_enabled: Option<bool>,
    pub(crate) engine: Option<String>,
    pub(crate) provider_base_url: Option<String>,
    pub(crate) provider_text_model: Option<String>,
    pub(crate) provider_vision_model: Option<String>,
    pub(crate) provider_offline: Option<bool>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SettingsUpdateResponse {
    pub(crate) message: String,
    pub(crate) applied_fields: Vec<String>,
    pub(crate) restart_required_fields: Vec<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub(crate) persist_failed: bool,
}

pub(crate) async fn get_settings(
    State(state): State<Arc<AppState>>,
) -> Result<Json<SettingsResponse>, AppError> {
    let cfg = state.config();
    Ok(Json(SettingsResponse {
        capture_enabled: cfg.capture.enabled,
        capture_interval_seconds: cfg.capture.interval_seconds,
        checkin_enabled: cfg.checkin.enabled,
        checkin_times: cfg.checkin_times_vec().to_vec(),
        checkin_jitter_minutes: cfg.checkin.jitter_minutes,
        conversation_inactivity_timeout_seconds: cfg.conversation.inactivity_timeout_seconds,
        conversation_auto_close_minutes: cfg.conversation.auto_close_minutes,
        goal_check_interval_seconds: cfg.goals.check_interval_seconds,
        mcp_enabled: cfg.mcp.enabled,
        engine: cfg.engine.engine.clone(),
        provider_base_url: cfg.engine.provider_base_url.clone(),
        provider_text_model: cfg.engine.provider_text_model.clone(),
        provider_vision_model: cfg.engine.provider_vision_model.clone(),
        provider_offline: cfg.engine.provider_offline,
    }))
}

pub(crate) async fn update_settings(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SettingsUpdateRequest>,
) -> Result<Json<SettingsUpdateResponse>, AppError> {
    let mut changes: HashMap<String, serde_json::Value> = HashMap::new();
    macro_rules! collect_opt {
        ($field:ident) => {
            if let Some(ref v) = body.$field {
                changes.insert(
                    stringify!($field).to_owned(),
                    serde_json::to_value(v).unwrap_or_default(),
                );
            }
        };
    }

    collect_opt!(capture_enabled);
    collect_opt!(capture_interval_seconds);
    collect_opt!(checkin_enabled);
    collect_opt!(checkin_jitter_minutes);
    collect_opt!(conversation_inactivity_timeout_seconds);
    collect_opt!(conversation_auto_close_minutes);
    collect_opt!(goal_check_interval_seconds);
    collect_opt!(mcp_enabled);
    collect_opt!(engine);
    collect_opt!(provider_base_url);
    collect_opt!(provider_text_model);
    collect_opt!(provider_vision_model);
    collect_opt!(provider_offline);

    if let Some(ref v) = body.checkin_times {
        changes.insert(
            "checkin.times".to_owned(),
            serde_json::to_value(v).unwrap_or_default(),
        );
    }

    if changes.is_empty() {
        return Ok(Json(SettingsUpdateResponse {
            message: "No changes provided".into(),
            applied_fields: vec![],
            restart_required_fields: vec![],
            persist_failed: false,
        }));
    }

    let result = state.config_manager.update(&changes);

    tracing::info!(
        applied = ?result.applied_fields,
        restart_required = ?result.restart_required_fields,
        persist_failed = result.persist_failed,
        "settings.updated"
    );

    let total = result.applied_fields.len() + result.restart_required_fields.len();
    Ok(Json(SettingsUpdateResponse {
        message: format!("Updated {total} setting(s)"),
        applied_fields: result.applied_fields,
        restart_required_fields: result.restart_required_fields,
        persist_failed: result.persist_failed,
    }))
}
