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
    /// Hot-swap; daemon rebuilds Copilot CLI and worker sessions on change.
    pub(crate) engine: String,
    pub(crate) provider_base_url: Option<String>,
    pub(crate) provider_chat_model: Option<String>,
    pub(crate) provider_batch_model: Option<String>,
    pub(crate) provider_vision_model: Option<String>,
    pub(crate) provider_chat_reasoning: Option<String>,
    pub(crate) provider_batch_reasoning: Option<String>,
    pub(crate) provider_vision_reasoning: Option<String>,
    pub(crate) provider_offline: bool,
    pub(crate) voice_enabled: bool,
    pub(crate) voice_persona: String,
    pub(crate) voice_speed: f32,
    /// User's primary STT language (BCP-47: "en", "zh"). Drives client-side
    /// engine selection in Mode B (FluidAudio). Field added in M6.A.
    pub(crate) voice_stt_language: String,
    /// End-of-utterance debounce preset. Kebab-case enum: "tight"|"balanced"|"patient".
    pub(crate) voice_pause_sensitivity: String,
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
    pub(crate) provider_chat_model: Option<String>,
    pub(crate) provider_batch_model: Option<String>,
    pub(crate) provider_vision_model: Option<String>,
    pub(crate) provider_chat_reasoning: Option<String>,
    pub(crate) provider_batch_reasoning: Option<String>,
    pub(crate) provider_vision_reasoning: Option<String>,
    pub(crate) provider_offline: Option<bool>,
    pub(crate) voice_enabled: Option<bool>,
    pub(crate) voice_persona: Option<String>,
    pub(crate) voice_speed: Option<f32>,
    pub(crate) voice_stt_language: Option<String>,
    pub(crate) voice_pause_sensitivity: Option<String>,
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
        provider_chat_model: cfg.engine.provider_chat_model.clone(),
        provider_batch_model: cfg.engine.provider_batch_model.clone(),
        provider_vision_model: cfg.engine.provider_vision_model.clone(),
        provider_chat_reasoning: cfg.engine.provider_chat_reasoning.clone(),
        provider_batch_reasoning: cfg.engine.provider_batch_reasoning.clone(),
        provider_vision_reasoning: cfg.engine.provider_vision_reasoning.clone(),
        provider_offline: cfg.engine.provider_offline,
        voice_enabled: cfg.voice.enabled,
        voice_persona: cfg.voice.persona.clone(),
        voice_speed: cfg.voice.speed,
        voice_stt_language: cfg.voice.stt_language.clone(),
        voice_pause_sensitivity: serde_json::to_value(cfg.voice.pause_sensitivity)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_else(|| "balanced".into()),
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
    collect_opt!(provider_chat_model);
    collect_opt!(provider_batch_model);
    collect_opt!(provider_vision_model);
    collect_opt!(provider_chat_reasoning);
    collect_opt!(provider_batch_reasoning);
    collect_opt!(provider_vision_reasoning);
    collect_opt!(provider_offline);
    collect_opt!(voice_enabled);
    collect_opt!(voice_persona);
    collect_opt!(voice_speed);
    collect_opt!(voice_stt_language);
    collect_opt!(voice_pause_sensitivity);

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
