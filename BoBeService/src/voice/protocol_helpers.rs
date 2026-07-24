//! WS protocol helpers shared across the voice handler split.
//!
//! Split out of `api/handlers/voice.rs` so the per-WS scope and the per-turn
//! task can both use these without each carrying a copy. Turn output remains
//! typed until the owning transport adapter serializes it.

use axum::extract::ws::{Message, Utf8Bytes, WebSocket};
use futures::{SinkExt, StreamExt};
use tracing::{debug, error};

use crate::speech::protocol::{ServerMessage, VoicePhase};
use crate::voice::output::VoiceOutput;

/// Send a single Error frame on a not-yet-installed WS and close it.
/// Used in the WS handshake reject path before `out_tx` exists.
pub(crate) async fn close_with_error(socket: WebSocket, code: &str, message: &str) {
    let (mut tx, _rx) = socket.split();
    let payload = ServerMessage::Error {
        code: code.to_string(),
        message: message.to_string(),
    };
    if let Ok(text) = serde_json::to_string(&payload) {
        if let Err(send_error) = tx.send(Message::Text(Utf8Bytes::from(text))).await {
            debug!(error = %send_error, "voice.handshake_error_send_failed");
        }
    } else {
        error!("voice.handshake_error_encode_failed");
    }
}

/// Emit a `state(phase)` event tagged with the current `turn_id`. Every
/// phase transition the daemon authors goes through this helper so client
/// state mirrors stay consistent.
pub(crate) async fn send_state(output: &VoiceOutput, phase: VoicePhase, turn_id: &str) {
    send_json(
        output,
        &ServerMessage::State {
            phase,
            turn_id: turn_id.to_string(),
        },
    )
    .await;
}

/// Emit a non-fatal Error frame (vs `close_with_error` which terminates the
/// socket). Used for protocol-level errors mid-session.
pub(crate) async fn send_error(output: &VoiceOutput, code: &str, message: &str) {
    send_json(
        output,
        &ServerMessage::Error {
            code: code.to_string(),
            message: message.to_string(),
        },
    )
    .await;
}

/// Push a typed control frame to the owning transport.
pub(crate) async fn send_json(output: &VoiceOutput, msg: &ServerMessage) {
    if !output.control(msg.clone()).await {
        debug!("voice.control_send_closed");
    }
}
