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
    /// User's primary language (BCP-47: `"en"`, `"zh"`, ...). Drives the
    /// Swift client's per-language STT engine pick (Parakeet for English,
    /// Qwen3-ASR for everything else). Daemon stores + echoes for tracing
    /// only — it never inspects the value in turn dispatch.
    /// v1 shipping: `"en"`, `"zh"`. Surfaced but not wired:
    /// `"es"`, `"el"`, `"ko"`, `"ja"`.
    pub(crate) stt_language: String,
    /// End-of-utterance debounce / silence tolerance preference.
    /// Maps to FluidAudio Parakeet's `eouDebounceMs` (English) and the
    /// Swift Qwen3 wrapper's VAD-driven silence timer (Mandarin), both
    /// consuming the same ms value. Daemon stores for round-trip.
    pub(crate) pause_sensitivity: PauseSensitivity,
    /// Whether the live partial-transcript caption ("VoicePartialCaption")
    /// is shown in the overlay while the user is speaking. Default on —
    /// gives instant feedback that BoBe heard the words; some users want
    /// it off because it can distract during long phrases.
    pub(crate) show_partial_caption: bool,
}

/// User-facing pause-sensitivity preset. Concrete millisecond mapping lives
/// in the engine layer so each provider can tune to its own characteristics.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum PauseSensitivity {
    /// 600ms — fast end-of-turn, can cut users off.
    Tight,
    /// 1280ms — default, matches FluidAudio Parakeet EOU model default.
    Balanced,
    /// 2000ms — favor waiting; good for thoughtful speakers / long pauses.
    Patient,
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            persona: "af_bella".into(),
            speed: 1.0,
            stt_language: "en".into(),
            pause_sensitivity: PauseSensitivity::Balanced,
            show_partial_caption: true,
        }
    }
}
