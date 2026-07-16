//! WS protocol helpers shared across the voice handler split.
//!
//! Split out of `api/handlers/voice.rs` so the per-WS scope and the per-turn
//! task can both use these without each carrying a copy. All four helpers
//! are thin wrappers over `axum`'s `ws::Message::Text`; the only state they
//! touch is the outbound `mpsc::Sender<Message>`.

use axum::extract::ws::{Message, Utf8Bytes, WebSocket};
use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tracing::{debug, error};

use crate::speech::protocol::{ServerMessage, VoicePhase};

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
pub(crate) async fn send_state(out_tx: &mpsc::Sender<Message>, phase: VoicePhase, turn_id: &str) {
    send_json(
        out_tx,
        &ServerMessage::State {
            phase,
            turn_id: turn_id.to_string(),
        },
    )
    .await;
}

/// Emit a non-fatal Error frame (vs `close_with_error` which terminates the
/// socket). Used for protocol-level errors mid-session.
pub(crate) async fn send_error(out_tx: &mpsc::Sender<Message>, code: &str, message: &str) {
    send_json(
        out_tx,
        &ServerMessage::Error {
            code: code.to_string(),
            message: message.to_string(),
        },
    )
    .await;
}

/// Serialise + push any `Serialize` value as a Text frame on the outbound
/// channel.
pub(crate) async fn send_json<T: serde::Serialize>(out_tx: &mpsc::Sender<Message>, msg: &T) {
    let text = match serde_json::to_string(msg) {
        Ok(text) => text,
        Err(encode_error) => {
            error!(error = %encode_error, "voice.control_encode_failed");
            return;
        }
    };
    if out_tx
        .send(Message::Text(Utf8Bytes::from(text)))
        .await
        .is_err()
    {
        debug!("voice.control_send_closed");
    }
}
