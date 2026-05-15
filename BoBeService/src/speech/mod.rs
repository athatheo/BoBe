//! Voice TTS engine + wire protocol.
//!
//! Mode B (client-side ASR via FluidAudio): the daemon is purely a text →
//! LLM → TTS pipeline. No STT, no VAD, no smart-turn live here — the
//! client owns all of that.
//!
//! What remains:
//!   - `protocol`           — WS DTOs
//!   - `providers::sherpa::kokoro_tts` — Kokoro TTS via sherpa-onnx
//!   - `tts`                — TTS trait
//!   - `sentence_buffer` + `markdown_strip` — TTS-input helpers

pub(crate) mod markdown_strip;
pub(crate) mod protocol;
pub(crate) mod providers;
pub(crate) mod sentence_buffer;
pub(crate) mod tts;

pub(crate) use tts::TtsEngine;
