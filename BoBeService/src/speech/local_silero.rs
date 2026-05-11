//! Silero v6.2.1 VAD wrapped by sherpa-onnx 1.13's `VoiceActivityDetector`.
//!
//! Why through sherpa-onnx and not ort-direct: we're already pulling sherpa-onnx
//! for STT + TTS, so reusing its onnxruntime instance avoids loading two engines
//! at startup. The wrapper handles frame buffering + speech-segment queue.

use std::path::Path;

use sherpa_onnx::{SileroVadModelConfig, VadModelConfig, VoiceActivityDetector};

use crate::error::AppError;
use crate::speech::vad::{AcousticVad, SpeechSegment};

/// Internal Silero buffer = 30s rolling window. Sherpa-onnx uses this for
/// the per-frame inference cache.
const BUFFER_SECONDS: f32 = 30.0;

pub(crate) struct LocalSileroVad {
    inner: VoiceActivityDetector,
}

impl LocalSileroVad {
    /// Load Silero v6.2.1 ONNX from `model_path`. Provider per platform
    /// (CoreML on macOS, CPU elsewhere); env `BOBE_VOICE_PROVIDER` overrides.
    pub(crate) fn load(
        model_path: &Path,
        provider: &str,
        num_threads: i32,
    ) -> Result<Self, AppError> {
        if !model_path.exists() {
            return Err(AppError::Config(format!(
                "Missing Silero VAD model: {}",
                model_path.display()
            )));
        }

        let silero = SileroVadModelConfig {
            model: Some(model_path.to_string_lossy().into_owned()),
            ..Default::default()
        };
        let config = VadModelConfig {
            silero_vad: silero,
            sample_rate: 16_000,
            num_threads,
            provider: Some(provider.to_string()),
            debug: false,
            ..Default::default()
        };

        let inner = VoiceActivityDetector::create(&config, BUFFER_SECONDS)
            .ok_or_else(|| AppError::Config("create Silero VAD failed".into()))?;

        Ok(Self { inner })
    }
}

impl AcousticVad for LocalSileroVad {
    fn accept(&self, samples: &[f32]) -> Result<(), AppError> {
        self.inner.accept_waveform(samples);
        Ok(())
    }

    fn has_segment(&self) -> bool {
        self.inner.detected()
    }

    fn pop_segment(&self) -> Option<SpeechSegment> {
        let segment = self.inner.front()?;
        let result = SpeechSegment {
            start_seconds: segment.start() as f32 / 16_000.0,
            samples: segment.samples().to_vec(),
        };
        self.inner.pop();
        Some(result)
    }

    fn flush(&self) {
        self.inner.flush();
    }

    fn reset(&self) {
        self.inner.reset();
    }
}
