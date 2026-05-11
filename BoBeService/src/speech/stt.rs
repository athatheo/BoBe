use crate::error::AppError;

/// STT result event. Phase-1 local-sherpa emits one Final after `is_endpoint()`.
/// Streaming-zipformer (M4 follow-up) will emit Partial chunks too.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) enum SttEvent {
    Partial(String),
    Final(String),
}

/// A loaded STT engine ready to transcribe PCM samples.
///
/// Implementations: see [`super::local_sherpa::LocalSherpaStt`].
/// Cloud impls (Azure, Deepgram) plug in here in a later commit.
pub(crate) trait SttEngine: Send + Sync {
    /// Transcribe a buffer of 16kHz mono PCM f32 samples. Synchronous because
    /// sherpa-rs is FFI-blocking; callers should wrap in `spawn_blocking`.
    fn transcribe(&self, samples: &[f32]) -> Result<String, AppError>;
}
