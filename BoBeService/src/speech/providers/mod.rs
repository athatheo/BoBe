//! Concrete provider implementations for the speech engine traits.
//!
//! Layout — one directory per provider:
//!   - `sherpa/` — sherpa-onnx Kokoro TTS (cross-platform; only used for
//!     daemon-side TTS in Mode B)
//!
//! Future providers (e.g. cloud TTS, Cartesia) slot in as sibling dirs.

pub(crate) mod sherpa;
