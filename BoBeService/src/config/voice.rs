//! Voice subsystem config — master toggle + Kokoro persona/speed defaults.
//! The Hello WS handshake's voice_id/speed override these per WS connection.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct VoiceConfig {
    /// Master toggle. When `false` the overlay hides the mic button and the
    /// daemon's `/voice/stream` returns an Error and closes.
    pub(crate) enabled: bool,
    /// Default Kokoro voice slot. Hello handshake's `voice_id` overrides
    /// this per WS connection. See `voice.rs::DEFAULT_KOKORO_VOICE`.
    pub(crate) persona: String,
    /// Default playback speed (0.5–2.0). Hello handshake's `speed` overrides
    /// this per WS connection. Outside the range gets clamped at synth time.
    pub(crate) speed: f32,
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            persona: "af_bella".into(),
            speed: 1.0,
        }
    }
}
