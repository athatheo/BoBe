//! WS endpoint `/voice/stream`. Phase 0.b shell: round-trip plumbing only.
//!
//! What this layer does today:
//!   - Accepts WS upgrade
//!   - Validates `hello` handshake (rate + codec)
//!   - Decodes incoming Opus frames into 16kHz f32 PCM, accumulates per-session
//!   - Routes JSON control messages (vad_hint, barge_in, wake, playback_ack, control)
//!   - Emits `state` transitions on connect / disconnect
//!
//! What lands later:
//!   0.c — wire Silero VAD per-frame + smart-turn gating + STT commit
//!   0.d — chat-pipeline observer + Kokoro sentence-streaming TTS
//!   M4.5.5 — barge-in 3-event protocol

use std::sync::Arc;

use axum::extract::State;
use axum::extract::ws::{Message, Utf8Bytes, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures::stream::SplitSink;
use futures::{SinkExt, StreamExt};
use opus::{Channels, Decoder as OpusDecoder};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::speech::protocol::{ClientMessage, ControlAction, ServerMessage, VoicePhase};

const OPUS_INPUT_SAMPLE_RATE: u32 = 16_000;
const TTS_OUTPUT_SAMPLE_RATE: u32 = 24_000;
const OPUS_MAX_FRAME_SAMPLES: usize = 2_880;

pub(crate) async fn voice_stream(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    if state.voice_stt.is_none() || state.voice_tts.is_none() || state.voice_vad.is_none() {
        warn!("voice.engines_unavailable");
        close_with_error(socket, "engines_unavailable", "voice models not installed").await;
        return;
    }

    let (mut tx, mut rx) = socket.split();
    let mut session: Option<VoiceSession> = None;

    while let Some(msg) = rx.next().await {
        let msg = match msg {
            Ok(m) => m,
            Err(e) => {
                warn!(error = %e, "voice.ws_recv_error");
                break;
            }
        };

        match msg {
            Message::Text(text) => {
                if !handle_control_text(text.as_str(), &mut tx, &mut session).await {
                    break;
                }
            }
            Message::Binary(bytes) => {
                if let Some(s) = session.as_mut() {
                    match s.decode_opus(&bytes) {
                        Ok(samples) => s.pcm.extend_from_slice(&samples),
                        Err(e) => warn!(error = %e, "voice.opus_decode_failed"),
                    }
                } else {
                    warn!("voice.binary_before_hello");
                }
            }
            Message::Close(_) => {
                info!("voice.close_received");
                break;
            }
            _ => {}
        }
    }

    if let Some(s) = session {
        info!(
            session = %s.session_id,
            pcm_samples = s.pcm.len(),
            "voice.disconnect"
        );
    }
}

/// Returns `false` to terminate the connection (after handshake errors).
async fn handle_control_text(
    text: &str,
    tx: &mut SplitSink<WebSocket, Message>,
    session: &mut Option<VoiceSession>,
) -> bool {
    let parsed: Result<ClientMessage, _> = serde_json::from_str(text);
    match parsed {
        Ok(ClientMessage::Hello {
            session_id,
            capture_rate,
            playback_rate,
            codec,
        }) => {
            info!(session = %session_id, capture_rate, playback_rate, codec, "voice.hello");
            if capture_rate != OPUS_INPUT_SAMPLE_RATE {
                send_error(
                    tx,
                    "rate_mismatch",
                    &format!("expected capture_rate={OPUS_INPUT_SAMPLE_RATE}"),
                )
                .await;
                return false;
            }
            if playback_rate != TTS_OUTPUT_SAMPLE_RATE {
                send_error(
                    tx,
                    "rate_mismatch",
                    &format!("expected playback_rate={TTS_OUTPUT_SAMPLE_RATE}"),
                )
                .await;
                return false;
            }
            if codec != "opus" {
                send_error(tx, "unsupported_codec", &format!("expected opus, got {codec}"))
                    .await;
                return false;
            }
            match VoiceSession::new(session_id) {
                Ok(s) => {
                    let turn_id = s.turn_id.clone();
                    *session = Some(s);
                    send_state(tx, VoicePhase::Listening, &turn_id).await;
                    true
                }
                Err(e) => {
                    error!(error = %e, "voice.session_init_failed");
                    send_error(tx, "session_init_failed", e).await;
                    false
                }
            }
        }
        Ok(ClientMessage::VadHint { .. }) => {
            // Phase 0.b shell — accepted, ignored. Daemon Silero is authoritative.
            true
        }
        Ok(ClientMessage::BargeIn {
            ts_ms,
            playback_ms_played,
        }) => {
            debug!(ts_ms, playback_ms_played, "voice.barge_in_received");
            // Real cancel + truncate lands with the M4.5.5 barge-in commit.
            true
        }
        Ok(ClientMessage::Wake {
            phrase,
            score,
            ts_ms,
        }) => {
            info!(phrase = %phrase, score, ts_ms, "voice.wake_received");
            // M5.1 territory; shell only logs.
            true
        }
        Ok(ClientMessage::PlaybackAck {
            chunk_id,
            played_ms,
        }) => {
            debug!(chunk_id, played_ms, "voice.playback_ack");
            // Truncation math; consumed by 0.d when sentence-streaming lands.
            true
        }
        Ok(ClientMessage::Control { action }) => {
            handle_control_action(action, tx, session).await;
            true
        }
        Err(e) => {
            warn!(error = %e, "voice.invalid_json");
            send_error(tx, "invalid_json", &format!("{e}")).await;
            true
        }
    }
}

async fn handle_control_action(
    action: ControlAction,
    tx: &mut SplitSink<WebSocket, Message>,
    session: &mut Option<VoiceSession>,
) {
    match action {
        ControlAction::Abort => {
            info!("voice.control.abort");
            if let Some(s) = session.as_ref() {
                send_state(tx, VoicePhase::Idle, &s.turn_id).await;
            }
        }
        ControlAction::Mute | ControlAction::Unmute | ControlAction::Reset => {
            debug!(?action, "voice.control_ignored_shell");
        }
    }
}

struct VoiceSession {
    session_id: String,
    turn_id: String,
    decoder: OpusDecoder,
    pcm: Vec<f32>,
}

impl VoiceSession {
    fn new(session_id: String) -> Result<Self, &'static str> {
        let decoder = OpusDecoder::new(OPUS_INPUT_SAMPLE_RATE, Channels::Mono)
            .map_err(|_| "create Opus decoder failed")?;
        Ok(Self {
            session_id,
            turn_id: format!("voice_{}", Uuid::new_v4().simple()),
            decoder,
            pcm: Vec::new(),
        })
    }

    fn decode_opus(&mut self, packet: &[u8]) -> Result<Vec<f32>, String> {
        let mut samples_i16 = vec![0_i16; OPUS_MAX_FRAME_SAMPLES];
        let n = self
            .decoder
            .decode(packet, &mut samples_i16, false)
            .map_err(|e| format!("opus decode: {e}"))?;
        samples_i16.truncate(n);
        Ok(samples_i16
            .iter()
            .map(|&s| f32::from(s) / 32_768.0)
            .collect())
    }
}

async fn close_with_error(socket: WebSocket, code: &str, message: &str) {
    let (mut tx, _rx) = socket.split();
    send_error(&mut tx, code, message).await;
}

async fn send_state(tx: &mut SplitSink<WebSocket, Message>, phase: VoicePhase, turn_id: &str) {
    send_json(
        tx,
        &ServerMessage::State {
            phase,
            turn_id: turn_id.to_string(),
        },
    )
    .await;
}

async fn send_error(tx: &mut SplitSink<WebSocket, Message>, code: &str, message: &str) {
    send_json(
        tx,
        &ServerMessage::Error {
            code: code.to_string(),
            message: message.to_string(),
        },
    )
    .await;
}

async fn send_json<T: serde::Serialize>(tx: &mut SplitSink<WebSocket, Message>, msg: &T) {
    let Ok(text) = serde_json::to_string(msg) else {
        return;
    };
    drop(tx.send(Message::Text(Utf8Bytes::from(text))).await);
}
