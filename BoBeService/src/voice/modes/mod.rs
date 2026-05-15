//! Voice mode dispatchers.
//!
//! Only Mode B (client-side ASR via FluidAudio) remains. The dispatcher
//! receives `ClientMessage::TranscriptFinal` over the WS and routes through
//! the shared convergence body (`voice/run_text_turn::run_text_turn`).

pub(crate) mod transcript_in;
