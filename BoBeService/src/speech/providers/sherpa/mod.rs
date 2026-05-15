//! sherpa-onnx-based provider impls. Only Kokoro TTS lives here — the
//! Zipformer/Silero/SmartTurn impls were removed in the Mode-B-only pivot
//! (M6.B no-fallback path) because the daemon no longer runs ASR.

pub(crate) mod kokoro_tts;
