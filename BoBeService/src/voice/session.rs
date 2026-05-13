//! Per-WS voice session state.
//!
//! Extracted from `api/handlers/voice.rs` so the WS handler stays focused
//! on the socket loop. `VoiceSession` owns everything that lives for one
//! WS connection: the Opus decoder, the per-turn join handle, the muted
//! flag, the per-WS Hello-handshake voice preferences, etc.
//!
//! `SessionVoiceConfig` and `VoiceDefaults` live here too — both are tiny
//! value types used by the session and the kokoro task.

use opus::{Channels, Decoder as OpusDecoder};
use tokio::task::JoinHandle;

use crate::app_state::AppState;

/// Sample rate for inbound Opus from the mic. Mirrored client-side in
/// `VoicePipeline.swift::captureSampleRate`. Wire-protocol contract; do
/// not change without coordinating across both stacks.
pub(crate) const OPUS_INPUT_SAMPLE_RATE: u32 = 16_000;
/// Outbound TTS Opus sample rate (Kokoro native). Mirrored client-side in
/// `VoicePipeline.swift::playbackSampleRate`.
pub(crate) const TTS_OUTPUT_SAMPLE_RATE: u32 = 24_000;
/// Upper bound on the number of samples a single Opus packet can decode to
/// at our 16 kHz input rate. 60 ms × 48 kHz worst-case scratch buffer for
/// the decoder; wire packets are 20 ms = 320 samples.
pub(crate) const OPUS_MAX_FRAME_SAMPLES: usize = 2_880;

/// Per-WS voice preferences carried in the Hello handshake. Stored on
/// `VoiceSession` and read by the kokoro task. Falls back to the daemon's
/// `voice.persona` / `voice.speed` defaults from `Config`, then to the
/// hard-coded constants if neither layer supplies a value.
#[derive(Clone)]
pub(crate) struct SessionVoiceConfig {
    pub(crate) voice_id: String,
    pub(crate) speed: f32,
}

impl SessionVoiceConfig {
    pub(crate) fn new(
        voice_id: Option<String>,
        speed: Option<f32>,
        defaults: &VoiceDefaults,
    ) -> Self {
        Self {
            voice_id: voice_id.unwrap_or_else(|| defaults.persona.clone()),
            speed: speed.unwrap_or(defaults.speed).clamp(0.5, 2.0),
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
    pub(crate) decoder: OpusDecoder,
    /// JoinHandle on the spawned `process_turn` task plus the turn_id it
    /// owns. `None` outside a turn; populated when audio commits, taken
    /// when a barge-in or natural completion releases it.
    pub(crate) current_turn: Option<TurnInFlight>,
    /// Client-requested mute — incoming audio frames are dropped before
    /// VAD while this is true. Toggled by Control{Mute|Unmute|Reset}.
    pub(crate) muted: bool,
    /// Count of speech segments dropped because a turn was already in
    /// flight when a new segment arrived. M5.2 hammering pushback will
    /// replace this drop policy with queueing/merge; until then we surface
    /// the count so real-world rates are visible.
    pub(crate) segments_dropped: u64,
    /// Latest `played_ms` from PlaybackAck. Used as a tighter floor for
    /// truncate offsets in barge-in when WS jitter delays the client's
    /// `barge_in.playback_ms_played` value.
    pub(crate) last_acked_played_ms: u64,
    /// Per-WS voice preferences from the Hello handshake. Falls through to
    /// defaults if the client didn't specify any.
    pub(crate) voice_cfg: SessionVoiceConfig,
    /// Most recent streaming-STT partial text, kept on the session so
    /// MinWords barge-in gating (C3) + cancel-phrase detection (C7) can
    /// inspect what the user has actually said so far this turn.
    pub(crate) last_partial_text: String,
}

pub(crate) struct TurnInFlight {
    pub(crate) turn_id: String,
    pub(crate) join: JoinHandle<()>,
}

impl VoiceSession {
    pub(crate) fn new(
        session_id: String,
        voice_cfg: SessionVoiceConfig,
    ) -> Result<Self, &'static str> {
        let decoder = OpusDecoder::new(OPUS_INPUT_SAMPLE_RATE, Channels::Mono)
            .map_err(|_| "create Opus decoder failed")?;
        Ok(Self {
            session_id,
            decoder,
            current_turn: None,
            muted: false,
            segments_dropped: 0,
            last_acked_played_ms: 0,
            voice_cfg,
            last_partial_text: String::new(),
        })
    }

    pub(crate) fn decode_opus(&mut self, packet: &[u8]) -> Result<Vec<f32>, String> {
        let mut samples_i16 = vec![0_i16; OPUS_MAX_FRAME_SAMPLES];
        let n = self
            .decoder
            .decode(packet, &mut samples_i16, false)
            .map_err(|e| format!("opus decode: {e}"))?;
        samples_i16.truncate(n);
        Ok(samples_i16
            .iter()
            .map(|&s| f32::from(s) / 32_768.0)
            .collect())
    }
}
