//! Text-prep filters consumed by `voice/sentence_pipeline.rs`. Lives under
//! `voice/` because every consumer is voice-side; `speech/` is reserved for
//! the wire protocol + TTS engine surface.

pub(crate) mod markdown_strip;
pub(crate) mod sentence_buffer;
