//! Read + patch the live config. Pre-pivot this handler exposed every
//! `BOBE_*` env var the daemon honored — backend selection, model
//! choice, embedding settings, learning intervals, goal worker
//! controls, similarity thresholds, memory retention. Most of those
//! knobs went away with the SDK pivot (no LLM backend choice — Copilot
//! CLI is the engine; no embeddings; no goal-worker subsystem; no
//! row-oriented memory). What remains is everything still load-bearing
//! at runtime.

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
    pub(crate) conversation_summary_enabled: bool,
    pub(crate) goal_check_interval_seconds: f64,
    pub(crate) mcp_enabled: bool,
    pub(crate) locale_override: Option<String>,
    pub(crate) effective_locale: String,
    pub(crate) supported_locales: Vec<String>,
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
    pub(crate) conversation_summary_enabled: Option<bool>,
    pub(crate) goal_check_interval_seconds: Option<f64>,
    pub(crate) mcp_enabled: Option<bool>,
    pub(crate) locale_override: Option<String>,
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
        conversation_summary_enabled: cfg.conversation.summary_enabled,
        goal_check_interval_seconds: cfg.goals.check_interval_seconds,
        mcp_enabled: cfg.mcp.enabled,
        locale_override: cfg.locale_override.clone(),
        effective_locale: cfg.effective_locale(),
        supported_locales: crate::i18n::SUPPORTED_LOCALES
            .iter()
            .map(|locale| (*locale).to_string())
            .collect(),
    }))
}

pub(crate) async fn update_settings(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SettingsUpdateRequest>,
) -> Result<Json<SettingsUpdateResponse>, AppError> {
    if let Some(ref locale) = body.locale_override {
        let normalized = locale.trim().replace('_', "-");
        if !normalized.is_empty()
            && !crate::i18n::SUPPORTED_LOCALES
                .iter()
                .any(|supported| *supported == normalized)
        {
            return Err(AppError::Validation(format!(
                "Invalid locale_override '{locale}'. Supported: {}",
                crate::i18n::SUPPORTED_LOCALES.join(", ")
            )));
        }
    }

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
    collect_opt!(conversation_summary_enabled);
    collect_opt!(goal_check_interval_seconds);
    collect_opt!(mcp_enabled);

    if let Some(ref locale) = body.locale_override {
        let normalized = locale.trim().replace('_', "-");
        if normalized.is_empty() {
            changes.insert("locale_override".to_owned(), serde_json::Value::Null);
        } else {
            changes.insert(
                "locale_override".to_owned(),
                serde_json::Value::String(normalized),
            );
        }
    }

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
