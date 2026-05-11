//! Semantic turn-detection trait — confirms "user is done speaking" by
//! looking at audio prosody + content, beyond pure-acoustic silence.
//!
//! Implementations:
//! - [`super::local_smart_turn::StubSmartTurn`] — always returns 1.0 (until
//!   the real Pipecat smart-turn-v3.1 ONNX impl lands with the VAD pipeline).
//! - Future: `OrtSmartTurn` via `ort 2.0` running the 8MB int8 ONNX.

use crate::error::AppError;

/// Predicts the probability that the user has finished speaking, given the
/// recent audio window.
///
/// Production threshold: 0.6-0.7 (favor "wait" over "cut off").
pub(crate) trait SemanticTurn: Send + Sync {
    /// Return P(turn complete) in `[0.0, 1.0]` for the given 16kHz mono PCM
    /// window (typically the last 8 seconds, zero-padded at the START if
    /// shorter, per Pipecat smart-turn convention).
    fn probability_complete(&self, samples: &[f32]) -> Result<f32, AppError>;
}
