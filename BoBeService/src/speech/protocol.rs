//! WS wire protocol — Deepgram-style: binary frames carry Opus packets,
//! text JSON on the same socket carries control. See docs/voice-stack-plan.md §3.

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ClientMessage {
    Start {
        sample_rate: u32,
        session: String,
    },
    Commit,
    Stop,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ServerMessage {
    Ack { session: String },
    Transcript {
        text: String,
        #[serde(rename = "final")]
        is_final: bool,
    },
    SpeakingStarted,
    SpeakingEnded,
    Error { message: String },
}
