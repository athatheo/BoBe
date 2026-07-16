//! Per-WS voice session state. Mode B (client owns ASR) — daemon never
//! decodes audio. Session tracks the per-WS preferences from Hello plus
//! the in-flight turn handle.

use std::time::Instant;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::app_state::AppState;

/// Outbound TTS Opus sample rate (Kokoro native). Single source at
/// `crate::constants::voice_wire::TTS_OUTPUT_SAMPLE_RATE`; re-exported
/// here so existing imports keep working.
pub(crate) use crate::constants::voice_wire::TTS_OUTPUT_SAMPLE_RATE;

/// Per-WS voice preferences carried in the Hello handshake. Stored on
/// `VoiceSession` and read by the kokoro task. Falls back to the daemon's
/// `voice.persona` / `voice.speed` defaults from `Config`.
#[derive(Clone)]
pub(crate) struct SessionVoiceConfig {
    pub(crate) voice_id: String,
    pub(crate) speed: f32,
    pub(crate) client_tts: bool,
}

impl SessionVoiceConfig {
    pub(crate) fn new(
        voice_id: Option<String>,
        speed: Option<f32>,
        tts_backend: Option<&str>,
        defaults: &VoiceDefaults,
    ) -> Self {
        Self {
            voice_id: voice_id.unwrap_or_else(|| defaults.persona.clone()),
            speed: speed.unwrap_or(defaults.speed).clamp(0.5, 2.0),
            client_tts: tts_backend == Some("client_supertonic"),
        }
    }
}

/// Snapshot of the daemon's voice defaults captured at WS-accept time.
/// Decoupled from `Config` so the WS scope doesn't hold an `ArcSwap` guard
/// across an `await`.
#[derive(Clone)]
pub(crate) struct VoiceDefaults {
    pub(crate) enabled: bool,
    pub(crate) persona: String,
    pub(crate) speed: f32,
}

impl VoiceDefaults {
    pub(crate) fn from_state(state: &AppState) -> Self {
        let cfg = state.config();
        Self {
            enabled: cfg.voice.enabled,
            persona: cfg.voice.persona.clone(),
            speed: cfg.voice.speed,
        }
    }
}

/// Per-WS state — one struct, lives in the handle_socket future scope.
pub(crate) struct VoiceSession {
    pub(crate) session_id: String,
    /// JoinHandle on the spawned per-turn task plus the turn_id it owns.
    /// `None` outside a turn; populated when `TranscriptFinal` admits a
    /// new turn, taken when barge-in or natural completion releases it.
    pub(crate) current_turn: Option<TurnInFlight>,
    /// Client-requested mute — the client should stop forwarding transcripts
    /// while true. The daemon still acknowledges the control message for
    /// UX symmetry, and incoming `TranscriptPartial`/`TranscriptFinal` are
    /// dropped while muted.
    pub(crate) muted: bool,
    /// Latest `played_ms` from PlaybackAck. Used as a tighter floor for
    /// truncate offsets in barge-in when WS jitter delays the client's
    /// `barge_in.playback_ms_played` value.
    pub(crate) last_acked_played_ms: u64,
    /// Per-WS voice preferences from the Hello handshake. Falls through to
    /// defaults if the client didn't specify any.
    pub(crate) voice_cfg: SessionVoiceConfig,
    /// Most recent client-pushed partial transcript. Drives the cancel-phrase
    /// regex + MinWords barge-in gate.
    pub(crate) last_partial_text: String,
    /// BCP-47 language from Hello (defaults to `"en"`). Carried for
    /// observability / tracing — daemon is language-agnostic otherwise.
    pub(crate) language: String,
}

pub(crate) struct TurnInFlight {
    pub(crate) turn_id: String,
    pub(crate) join: JoinHandle<()>,
    pub(crate) started_at: Instant,
    pub(crate) playback_complete: Option<oneshot::Sender<()>>,
}

impl VoiceSession {
    pub(crate) fn new(session_id: String, voice_cfg: SessionVoiceConfig, language: String) -> Self {
        Self {
            session_id,
            current_turn: None,
            muted: false,
            last_acked_played_ms: 0,
            voice_cfg,
            last_partial_text: String::new(),
            language,
        }
    }
}
