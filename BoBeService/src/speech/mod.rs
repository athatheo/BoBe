//! TTS engine + wire protocol. Mode B = daemon does TTS only; STT/VAD
//! live client-side. `protocol` = WS DTOs, `providers::sherpa::kokoro_tts`
//! = Kokoro via sherpa-onnx, `tts` = trait.

pub(crate) mod protocol;
pub(crate) mod providers;
pub(crate) mod tts;

pub(crate) use tts::TtsEngine;
