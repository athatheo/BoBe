//! Voice-handler-side modules — distinct from `speech/` which holds the
//! engine traits + their model wrappers. Files here are about wiring
//! engines into the WS turn loop: pre-rendered fillers, sink registry
//! (M4.5.0c+), telemetry (D1), proactive-routing scaffold (M5.2).

pub(crate) mod cancel_phrases;
pub(crate) mod context;
pub(crate) mod control;
pub(crate) mod engines;
pub(crate) mod filler_library;
pub(crate) mod install_service;
pub(crate) mod opus;
pub(crate) mod protocol_helpers;
pub(crate) mod sentence_pipeline;
pub(crate) mod session;
pub(crate) mod sinks;
pub(crate) mod telemetry;
pub(crate) mod turn_flow;
