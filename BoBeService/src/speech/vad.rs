//! Acoustic VAD trait — gates the mic with frame-level speech detection.
//!
//! Implementations: [`super::local_silero::LocalSileroVad`] (Silero v6.2.1
//! via sherpa-onnx). Future: cloud impls slot in here.

use crate::error::AppError;

/// One contiguous speech segment, in the audio timeline of pushed samples.
#[derive(Debug, Clone)]
pub(crate) struct SpeechSegment {
    /// Start time within the pushed-audio timeline, in seconds.
    pub(crate) start_seconds: f32,
    /// Mono f32 PCM samples at the engine's native sample rate (16kHz).
    pub(crate) samples: Vec<f32>,
}

/// Frame-level VAD that buffers audio and emits complete speech segments.
///
/// All methods take `&self` because the underlying wrapper owns internal
/// mutability. Implementations must be `Send + Sync`.
pub(crate) trait AcousticVad: Send + Sync {
    /// Push 16kHz mono f32 PCM samples. Internally buffered + analyzed.
    fn accept(&self, samples: &[f32]) -> Result<(), AppError>;

    /// True once a complete speech segment is queued for retrieval.
    fn has_segment(&self) -> bool;

    /// Take the oldest queued speech segment, or `None` if none ready.
    fn pop_segment(&self) -> Option<SpeechSegment>;

    /// Force-flush any pending speech (e.g., on turn-end signal from above).
    fn flush(&self);

    /// Reset internal state — call between conversation turns.
    fn reset(&self);
}
