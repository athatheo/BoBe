//! Semantic turn-detection: stub that always returns 1.0 (complete).
//!
//! Real impl with Pipecat smart-turn-v3.1 (8MB int8 ONNX via `ort 2.0`) lands
//! with the VAD pipeline. Whisper-style mel preprocessing must be reimplemented
//! in Rust because the ONNX graph expects pre-extracted features.
//!
//! Using the stub means we depend on acoustic Silero alone for turn-end
//! detection — falls back to 700ms silence floor + first-enabled-soul voice
//! persona until the real smart-turn lands.

use crate::error::AppError;
use crate::speech::turn::SemanticTurn;

pub(crate) struct StubSmartTurn;

impl SemanticTurn for StubSmartTurn {
    fn probability_complete(&self, _samples: &[f32]) -> Result<f32, AppError> {
        Ok(1.0)
    }
}
