//! Voice stack — STT + TTS + acoustic VAD + semantic-turn engines + WS protocol.
//!
//! Layered per direction: separate `SttEngine`, `TtsEngine`, `AcousticVad`,
//! `SemanticTurn` traits. Sherpa-onnx 1.13 wraps STT (Moonshine), TTS (Kokoro),
//! and VAD (Silero v6.2.1 via `VoiceActivityDetector`). Smart-turn (when real)
//! loads via `ort 2.0` directly because it's not in sherpa-onnx.

pub(crate) mod local_kokoro;
pub(crate) mod local_sherpa;
pub(crate) mod local_silero;
pub(crate) mod local_smart_turn;
pub(crate) mod protocol;
pub(crate) mod stt;
pub(crate) mod tts;
pub(crate) mod turn;
pub(crate) mod vad;

pub(crate) use stt::SttEngine;
pub(crate) use tts::TtsEngine;
pub(crate) use turn::SemanticTurn;
pub(crate) use vad::AcousticVad;
