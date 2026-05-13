//! Voice stack — STT + TTS + acoustic VAD + semantic-turn engines + WS protocol.
//!
//! Layered per direction: separate `SttEngine`, `TtsEngine`, `AcousticVad`,
//! `SemanticTurn` traits. Sherpa-onnx 1.13 wraps STT (Moonshine), TTS (Kokoro),
//! and VAD (Silero v6.2.1 via `VoiceActivityDetector`). Smart-turn (when real)
//! loads via `ort 2.0` directly because it's not in sherpa-onnx.

pub(crate) mod local_kokoro;
pub(crate) mod local_silero;
pub(crate) mod markdown_strip;
pub(crate) mod protocol;
pub(crate) mod sentence_buffer;
pub(crate) mod smart_turn_onnx;
pub(crate) mod streaming_stt;
pub(crate) mod tts;
pub(crate) mod turn;
pub(crate) mod vad;

pub(crate) use streaming_stt::StreamingSttEngine;
pub(crate) use tts::TtsEngine;
pub(crate) use turn::SemanticTurn;
pub(crate) use vad::AcousticVad;
