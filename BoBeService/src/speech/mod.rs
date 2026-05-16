//! Voice TTS engine + wire protocol.
//!
//! Mode B (client-side ASR via FluidAudio): the daemon is purely a text →
//! LLM → TTS pipeline. No STT, no VAD, no smart-turn live here — the
//! client owns all of that. Text-prep filters (markdown_strip,
//! sentence_buffer) moved to `voice/text_prep/` since they're voice-
//! pipeline inputs, not part of the speech wire/engine surface.
//!
//! What remains:
//!   - `protocol`                       — WS DTOs
//!   - `providers::sherpa::kokoro_tts`  — Kokoro TTS via sherpa-onnx
//!   - `tts`                            — TTS trait

pub(crate) mod protocol;
pub(crate) mod providers;
pub(crate) mod tts;

pub(crate) use tts::TtsEngine;
