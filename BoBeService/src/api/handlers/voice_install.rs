//! `/voice/install` endpoints. Drives the daemon-owned voice-model
//! installer (replaces the legacy `scripts/install-voice-models.sh`):
//!
//!   GET    /voice/install/status   — current snapshot + per-model presence
//!   POST   /voice/install/start    — kick off (sequential downloads)
//!   POST   /voice/install/cancel   — best-effort cancel of in-flight job
//!
//! Welcome wizard + Settings → Voice surface these. Progress is poll-based
//! today; the snapshot is cheap to render (4 models × a handful of fields).

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Serialize;

use crate::app_state::AppState;
use crate::voice::install_service::{
    InstallStatus, ModelProgress, VoiceInstallSnapshot, VoiceModelKind,
};

#[derive(Serialize)]
pub(crate) struct InstallStatusResponse {
    /// Aggregate state across all models (running / complete / failed).
    pub status: InstallStatus,
    /// Per-model progress entries, fixed order matching VoiceModelKind::all().
    pub models: Vec<ModelProgress>,
    /// Snapshot of on-disk presence — orthogonal to `status` because
    /// snapshot.status can be Idle while all four models are already
    /// present from a previous run.
    pub installed: PresenceSnapshot,
}

#[derive(Serialize)]
pub(crate) struct PresenceSnapshot {
    pub streaming_stt: bool,
    pub tts: bool,
    pub vad: bool,
    pub smart_turn: bool,
    pub all_present: bool,
}

pub(crate) async fn status(State(state): State<Arc<AppState>>) -> Json<InstallStatusResponse> {
    let snap: VoiceInstallSnapshot = state.voice_install.subscribe().await.borrow().clone();
    let install = state.voice_install.as_ref();
    let presence = PresenceSnapshot {
        streaming_stt: install.is_installed(VoiceModelKind::StreamingStt),
        tts: install.is_installed(VoiceModelKind::Tts),
        vad: install.is_installed(VoiceModelKind::Vad),
        smart_turn: install.is_installed(VoiceModelKind::SmartTurn),
        all_present: install.is_installed(VoiceModelKind::StreamingStt)
            && install.is_installed(VoiceModelKind::Tts)
            && install.is_installed(VoiceModelKind::Vad)
            && install.is_installed(VoiceModelKind::SmartTurn),
    };
    Json(InstallStatusResponse {
        status: snap.status,
        models: snap.models,
        installed: presence,
    })
}

pub(crate) async fn start(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.voice_install.start().await {
        Ok(()) => StatusCode::ACCEPTED.into_response(),
        Err(crate::error::AppError::Conflict(msg)) => {
            (StatusCode::CONFLICT, msg).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub(crate) async fn cancel(State(state): State<Arc<AppState>>) -> StatusCode {
    state.voice_install.cancel().await;
    StatusCode::ACCEPTED
}
