//! WS endpoint `/voice/stream`. Phase 0.c: daemon VAD pipeline + STT commit.
//!
//! What this layer does:
//!   - Handshake (rate + codec validation)
//!   - Decode incoming Opus → 16kHz f32 PCM frames
//!   - Push every frame into Silero VAD (daemon-side, authoritative)
//!   - When Silero queues a speech segment, gate via smart-turn (stub passes
//!     all for now; real ONNX in M4.5.2)
//!   - Acquire UserMessageGuard, run STT on the segment, emit `transcript.final`
//!   - Phase 0.c stops there — LLM + Kokoro TTS arrive in 0.d
//!
//! State transitions emitted today:
//!   on hello             → state(Listening)
//!   on segment ready     → state(Thinking) with fresh turn_id
//!   on transcript done   → state(Listening)
//!   on control(Abort)    → state(Idle)

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
use crate::runtime::session::RuntimeSession;
use crate::speech::protocol::{ClientMessage, ControlAction, ServerMessage, VoicePhase};
use crate::speech::{AcousticVad, SemanticTurn, SttEngine, TtsEngine};

const OPUS_INPUT_SAMPLE_RATE: u32 = 16_000;
const TTS_OUTPUT_SAMPLE_RATE: u32 = 24_000;
const OPUS_MAX_FRAME_SAMPLES: usize = 2_880;

/// Smart-turn threshold — segment is treated as a complete turn iff
/// `probability_complete >= TURN_COMPLETE_THRESHOLD`. The stub always returns
/// 1.0 so this is effectively a no-op until the real ONNX impl lands.
const TURN_COMPLETE_THRESHOLD: f32 = 0.7;

pub(crate) async fn voice_stream(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

struct VoiceEngines {
    stt: Arc<dyn SttEngine>,
    #[allow(dead_code, reason = "used by Kokoro pipeline in 0.d")]
    tts: Arc<dyn TtsEngine>,
    vad: Arc<dyn AcousticVad>,
    smart_turn: Arc<dyn SemanticTurn>,
}

impl VoiceEngines {
    fn from_state(state: &AppState) -> Option<Self> {
        Some(Self {
            stt: Arc::clone(state.voice_stt.as_ref()?),
            tts: Arc::clone(state.voice_tts.as_ref()?),
            vad: Arc::clone(state.voice_vad.as_ref()?),
            smart_turn: Arc::clone(state.voice_smart_turn.as_ref()?),
        })
    }
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let Some(engines) = VoiceEngines::from_state(&state) else {
        warn!("voice.engines_unavailable");
        close_with_error(socket, "engines_unavailable", "voice models not installed").await;
        return;
    };

    let (mut tx, mut rx) = socket.split();
    let mut session: Option<VoiceSession> = None;
    let runtime_session = Arc::clone(&state.runtime_session);

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
                let Some(s) = session.as_mut() else {
                    warn!("voice.binary_before_hello");
                    continue;
                };
                let samples = match s.decode_opus(&bytes) {
                    Ok(samples) => samples,
                    Err(e) => {
                        warn!(error = %e, "voice.opus_decode_failed");
                        continue;
                    }
                };
                if let Err(e) = engines.vad.accept(&samples) {
                    warn!(error = %e, "voice.vad_accept_failed");
                    continue;
                }
                while engines.vad.has_segment() {
                    if let Some(segment) = engines.vad.pop_segment() {
                        process_segment(
                            segment,
                            s,
                            &engines,
                            &runtime_session,
                            &mut tx,
                        )
                        .await;
                    }
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
        info!(session = %s.session_id, "voice.disconnect");
    }
}

async fn process_segment(
    segment: crate::speech::vad::SpeechSegment,
    session: &mut VoiceSession,
    engines: &VoiceEngines,
    runtime_session: &Arc<RuntimeSession>,
    tx: &mut SplitSink<WebSocket, Message>,
) {
    // Semantic turn gate — discard segment if the user isn't really done
    // speaking. Stub returns 1.0, so this never trips in Phase 0.c.
    match engines.smart_turn.probability_complete(&segment.samples) {
        Ok(p) if p < TURN_COMPLETE_THRESHOLD => {
            debug!(p, "voice.smart_turn_incomplete_skip");
            return;
        }
        Ok(p) => debug!(p, "voice.smart_turn_complete"),
        Err(e) => {
            warn!(error = %e, "voice.smart_turn_failed_passthrough");
        }
    }

    // Acquire single-flight guard so text + voice can't race on the chat
    // worker. Conflict → emit error + keep listening.
    let guard = match runtime_session.try_begin_user_message() {
        Ok(g) => g,
        Err(reason) => {
            send_error(tx, "conflict", reason).await;
            return;
        }
    };

    let new_turn_id = format!("voice_{}", Uuid::new_v4().simple());
    session.turn_id = new_turn_id.clone();
    send_state(tx, VoicePhase::Thinking, &new_turn_id).await;

    let stt = Arc::clone(&engines.stt);
    let samples = segment.samples;
    let stt_result = tokio::task::spawn_blocking(move || stt.transcribe(&samples)).await;

    let transcript = match stt_result {
        Ok(Ok(text)) => text,
        Ok(Err(e)) => {
            error!(error = %e, "voice.stt_failed");
            send_error(tx, "stt_failed", &e.to_string()).await;
            send_state(tx, VoicePhase::Listening, &new_turn_id).await;
            drop(guard);
            return;
        }
        Err(e) => {
            error!(error = %e, "voice.stt_join_failed");
            send_error(tx, "stt_join_failed", &e.to_string()).await;
            send_state(tx, VoicePhase::Listening, &new_turn_id).await;
            drop(guard);
            return;
        }
    };

    let trimmed = transcript.trim();
    if trimmed.is_empty() {
        // Silent drop: no orphan turn in chat history; just a friendly nudge.
        send_error(tx, "empty_transcript", "I didn't catch that").await;
    } else {
        info!(turn = %new_turn_id, transcript = %trimmed, "voice.transcript_final");
        send_json(
            tx,
            &ServerMessage::TranscriptFinal {
                turn_id: new_turn_id.clone(),
                text: trimmed.to_string(),
            },
        )
        .await;
        // 0.d will hand the transcript to runtime_session.handle_user_message
        // with a sentence-buffering observer that pipes to Kokoro TTS. Today
        // we stop after transcript.final and return to listening.
    }

    send_state(tx, VoicePhase::Listening, &new_turn_id).await;
    drop(guard);
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
                send_error(tx, "unsupported_codec", &format!("expected opus, got {codec}")).await;
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
            // Accepted, ignored — daemon Silero is authoritative.
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
            true
        }
        Ok(ClientMessage::PlaybackAck {
            chunk_id,
            played_ms,
        }) => {
            debug!(chunk_id, played_ms, "voice.playback_ack");
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
}

impl VoiceSession {
    fn new(session_id: String) -> Result<Self, &'static str> {
        let decoder = OpusDecoder::new(OPUS_INPUT_SAMPLE_RATE, Channels::Mono)
            .map_err(|_| "create Opus decoder failed")?;
        Ok(Self {
            session_id,
            turn_id: format!("voice_{}", Uuid::new_v4().simple()),
            decoder,
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
