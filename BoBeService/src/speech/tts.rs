use crate::error::AppError;

/// A loaded TTS engine ready to synthesize text to PCM audio.
///
/// Implementations: see [`super::providers::sherpa::kokoro_tts::LocalKokoroTts`].
/// Cloud impls (Azure Neural, ElevenLabs) plug in here in a later commit.
pub(crate) trait TtsEngine: Send + Sync {
    /// Synthesize `text` with the given voice + speed. Returns f32 PCM at the
    /// engine's native sample rate (see `sample_rate()`). Synchronous —
    /// callers should wrap in `spawn_blocking`.
    fn synthesize(&self, text: &str, voice: &str, speed: f32) -> Result<Vec<f32>, AppError>;

    /// Native sample rate of the synthesized PCM (Kokoro = 24000).
    fn sample_rate(&self) -> u32;
}
