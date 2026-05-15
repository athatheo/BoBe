//! Voice-handler-side modules — distinct from `speech/` which holds the
//! Kokoro TTS engine. Files here are about wiring TTS into the WS turn
//! loop: per-WS session state, per-turn convergence body, pre-rendered
//! filler library, sink registry, telemetry counters.

pub(crate) mod cancel_phrases;
pub(crate) mod context;
pub(crate) mod control;
pub(crate) mod engines;
pub(crate) mod filler_library;
pub(crate) mod install_artifacts;
pub(crate) mod install_extract;
pub(crate) mod install_service;
pub(crate) mod modes;
pub(crate) mod opus;
pub(crate) mod protocol_helpers;
pub(crate) mod run_text_turn;
pub(crate) mod sentence_pipeline;
pub(crate) mod session;
pub(crate) mod sinks;
pub(crate) mod telemetry;
pub(crate) mod turn_flow;
