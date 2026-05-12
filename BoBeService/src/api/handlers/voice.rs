//! WS endpoint `/voice/stream`. Phase 0.d: end-to-end voice turn.
//!
//! Flow on a complete turn:
//!   1. Handshake → state(Listening)
//!   2. audio.in binary → Opus decode → Silero VAD per-frame
//!   3. Silero emits SpeechSegment → smart-turn gate → STT
//!   4. transcript.final → state(Thinking) → state(Speaking)
//!   5. Call runtime_session.handle_user_message_with_observer with a
//!      sentence-buffer observer. Observer pipes deltas → markdown_stripper
//!      → sentence_buffer → mpsc to a Kokoro spawn_blocking consumer.
//!   6. Kokoro task synthesises per sentence → Opus 20ms frames → binary
//!      tts.chunk over WS (single dedicated WS writer task; both the main
//!      loop and Kokoro task post Messages via mpsc).
//!   7. LLM stream ends → flush sentence buffer → close sentence_tx → Kokoro
//!      drains → tts.end → state(Listening) → drop UserMessageGuard.
//!
//! Still TODO in M4.5.x:
//!   - Barge-in (0.d is sequential — no mid-turn interrupt yet)
//!   - Cached filler audio at 800ms TTFT
//!   - Stall watchdog
//!   - Wake-word integration

use std::sync::Arc;

use axum::extract::State;
use axum::extract::ws::{Message, Utf8Bytes, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};
use opus::{Application, Channels, Decoder as OpusDecoder, Encoder as OpusEncoder};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::runtime::session::RuntimeSession;
use crate::speech::markdown_strip::MarkdownStripper;
use crate::speech::protocol::{
    encode_tts_frame, ClientMessage, ControlAction, ServerMessage, VoicePhase, FLAG_FIRST_OF_TURN,
};
use crate::speech::sentence_buffer::SentenceBuffer;
use crate::speech::{AcousticVad, SemanticTurn, SttEngine, TtsEngine};

const OPUS_INPUT_SAMPLE_RATE: u32 = 16_000;
const TTS_OUTPUT_SAMPLE_RATE: u32 = 24_000;
const OPUS_MAX_FRAME_SAMPLES: usize = 2_880;
const TTS_OPUS_BITRATE_BPS: i32 = 24_000;
const OUTBOUND_CHANNEL_CAPACITY: usize = 64;
const SENTENCE_CHANNEL_CAPACITY: usize = 4;

/// Smart-turn pass-through threshold. Stub returns 1.0 → trivially passes;
/// real smart-turn v3.1 (M4.5.2) will gate here.
const TURN_COMPLETE_THRESHOLD: f32 = 0.7;

/// Default voice slot — replaced by per-soul `voice_id` setting in M4.5.0c.
const DEFAULT_KOKORO_VOICE: &str = "af_bella";
const DEFAULT_KOKORO_SPEED: f32 = 1.0;

pub(crate) async fn voice_stream(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

struct VoiceEngines {
    stt: Arc<dyn SttEngine>,
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

#[allow(
    clippy::collapsible_match,
    reason = "outer match has Binary/Close arms that prevent if-let collapse; \
              async call also disallows match guards"
)]
async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let Some(engines) = VoiceEngines::from_state(&state) else {
        warn!("voice.engines_unavailable");
        close_with_error(socket, "engines_unavailable", "voice models not installed").await;
        return;
    };

    let (ws_tx, mut rx) = socket.split();
    let (out_tx, mut out_rx) = mpsc::channel::<Message>(OUTBOUND_CHANNEL_CAPACITY);

    // Dedicated WS writer task — sole owner of ws_tx. Both the main loop and
    // the per-turn Kokoro task fan messages in via cloned out_tx senders.
    let writer = tokio::spawn(async move {
        let mut ws_tx = ws_tx;
        while let Some(msg) = out_rx.recv().await {
            if ws_tx.send(msg).await.is_err() {
                break;
            }
        }
        drop(ws_tx.close().await);
    });

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
                if !handle_control_text(text.as_str(), &out_tx, &mut session).await {
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
                        process_segment(segment, s, &engines, &runtime_session, &out_tx).await;
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
    drop(out_tx);
    drop(writer.await);
}

async fn process_segment(
    segment: crate::speech::vad::SpeechSegment,
    session: &mut VoiceSession,
    engines: &VoiceEngines,
    runtime_session: &Arc<RuntimeSession>,
    out_tx: &mpsc::Sender<Message>,
) {
    // Semantic turn gate — discard if the user isn't really done yet.
    match engines.smart_turn.probability_complete(&segment.samples) {
        Ok(p) if p < TURN_COMPLETE_THRESHOLD => {
            debug!(p, "voice.smart_turn_incomplete_skip");
            return;
        }
        Ok(p) => debug!(p, "voice.smart_turn_complete"),
        Err(e) => warn!(error = %e, "voice.smart_turn_failed_passthrough"),
    }

    let guard = match runtime_session.try_begin_user_message() {
        Ok(g) => g,
        Err(reason) => {
            send_error(out_tx, "conflict", reason).await;
            return;
        }
    };

    let new_turn_id = format!("voice_{}", Uuid::new_v4().simple());
    session.turn_id.clone_from(&new_turn_id);
    send_state(out_tx, VoicePhase::Thinking, &new_turn_id).await;

    // STT
    let stt = Arc::clone(&engines.stt);
    let samples = segment.samples;
    let stt_result = tokio::task::spawn_blocking(move || stt.transcribe(&samples)).await;
    let transcript = match stt_result {
        Ok(Ok(text)) => text,
        Ok(Err(e)) => {
            error!(error = %e, "voice.stt_failed");
            send_error(out_tx, "stt_failed", &e.to_string()).await;
            send_state(out_tx, VoicePhase::Listening, &new_turn_id).await;
            drop(guard);
            return;
        }
        Err(e) => {
            error!(error = %e, "voice.stt_join_failed");
            send_error(out_tx, "stt_join_failed", &e.to_string()).await;
            send_state(out_tx, VoicePhase::Listening, &new_turn_id).await;
            drop(guard);
            return;
        }
    };

    let trimmed = transcript.trim();
    if trimmed.is_empty() {
        send_error(out_tx, "empty_transcript", "I didn't catch that").await;
        send_state(out_tx, VoicePhase::Listening, &new_turn_id).await;
        drop(guard);
        return;
    }
    info!(turn = %new_turn_id, transcript = %trimmed, "voice.transcript_final");
    send_json(
        out_tx,
        &ServerMessage::TranscriptFinal {
            turn_id: new_turn_id.clone(),
            text: trimmed.to_string(),
        },
    )
    .await;

    // Spin up the per-turn TTS pipeline
    send_state(out_tx, VoicePhase::Speaking, &new_turn_id).await;
    let (sentence_tx, sentence_rx) = mpsc::channel::<String>(SENTENCE_CHANNEL_CAPACITY);
    let kokoro_task = spawn_kokoro_task(
        Arc::clone(&engines.tts),
        sentence_rx,
        out_tx.clone(),
        new_turn_id.clone(),
    );

    let pipeline = Arc::new(std::sync::Mutex::new(SentencePipeline::new(
        sentence_tx.clone(),
    )));
    let pipeline_for_obs = Arc::clone(&pipeline);
    let observer = move |delta: &str| {
        if let Ok(mut p) = pipeline_for_obs.lock() {
            p.feed(delta);
        }
    };

    // Hand the transcript to the chat pipeline with our sentence-buffering
    // observer. This persists the user voice turn, runs the SDK Chat worker,
    // streams text_delta SSE for the overlay UI, and pipes the same tokens
    // into our pipeline → Kokoro task.
    runtime_session
        .handle_user_message_with_observer(trimmed, &new_turn_id, observer)
        .await;

    // Flush any trailing sentence (LLM may have ended without trailing punctuation)
    if let Ok(mut p) = pipeline.lock() {
        p.flush();
    }
    // Drop the sender so the Kokoro task drains and exits
    drop(pipeline);
    drop(sentence_tx);
    drop(kokoro_task.await);

    send_json(
        out_tx,
        &ServerMessage::TtsEnd {
            turn_id: new_turn_id.clone(),
        },
    )
    .await;
    send_state(out_tx, VoicePhase::Listening, &new_turn_id).await;
    drop(guard);
}

struct SentencePipeline {
    md: MarkdownStripper,
    sb: SentenceBuffer,
    tx: mpsc::Sender<String>,
}

impl SentencePipeline {
    fn new(tx: mpsc::Sender<String>) -> Self {
        Self {
            md: MarkdownStripper::new(),
            sb: SentenceBuffer::new(),
            tx,
        }
    }

    fn feed(&mut self, delta: &str) {
        let clean = self.md.feed(delta);
        if clean.is_empty() {
            return;
        }
        for sentence in self.sb.feed(&clean) {
            drop(self.tx.try_send(sentence));
        }
    }

    fn flush(&mut self) {
        for sentence in self.sb.flush() {
            drop(self.tx.try_send(sentence));
        }
    }
}

fn spawn_kokoro_task(
    tts: Arc<dyn TtsEngine>,
    mut sentence_rx: mpsc::Receiver<String>,
    out_tx: mpsc::Sender<Message>,
    _turn_id: String,
) -> tokio::task::JoinHandle<()> {
    let sample_rate = tts.sample_rate();
    tokio::spawn(async move {
        let mut chunk_id: u64 = 0;
        let mut first_chunk_pending = true;
        while let Some(sentence) = sentence_rx.recv().await {
            let tts_clone = Arc::clone(&tts);
            let sentence_owned = sentence.clone();
            let synth = tokio::task::spawn_blocking(move || {
                tts_clone.synthesize(&sentence_owned, DEFAULT_KOKORO_VOICE, DEFAULT_KOKORO_SPEED)
            })
            .await;
            let pcm = match synth {
                Ok(Ok(pcm)) => pcm,
                Ok(Err(e)) => {
                    warn!(error = %e, "voice.kokoro_synth_failed");
                    continue;
                }
                Err(e) => {
                    warn!(error = %e, "voice.kokoro_join_failed");
                    continue;
                }
            };
            for opus_packet in encode_opus_frames(&pcm, sample_rate) {
                let mut flags = 0_u8;
                if first_chunk_pending {
                    flags |= FLAG_FIRST_OF_TURN;
                    first_chunk_pending = false;
                }
                let framed = encode_tts_frame(chunk_id, flags, &opus_packet);
                chunk_id = chunk_id.saturating_add(1);
                if out_tx.send(Message::Binary(framed.into())).await.is_err() {
                    return;
                }
            }
        }
    })
}

fn encode_opus_frames(pcm: &[f32], sample_rate: u32) -> Vec<Vec<u8>> {
    let mut encoder = match OpusEncoder::new(sample_rate, Channels::Mono, Application::Voip) {
        Ok(e) => e,
        Err(e) => {
            warn!(error = %e, "voice.opus_encoder_failed");
            return Vec::new();
        }
    };
    if let Err(e) = encoder.set_bitrate(opus::Bitrate::Bits(TTS_OPUS_BITRATE_BPS)) {
        warn!(error = %e, "voice.opus_bitrate_failed");
    }
    let frame_samples = (sample_rate as usize) / 50; // 20ms
    let mut frames = Vec::new();
    for chunk in pcm.chunks(frame_samples) {
        let mut input = chunk.to_vec();
        if input.len() < frame_samples {
            input.resize(frame_samples, 0.0);
        }
        let pcm_i16: Vec<i16> = input
            .iter()
            .map(|&s| (s.clamp(-1.0, 1.0) * 32_767.0) as i16)
            .collect();
        let mut out = vec![0_u8; 1500];
        match encoder.encode(&pcm_i16, &mut out) {
            Ok(n) => {
                out.truncate(n);
                frames.push(out);
            }
            Err(e) => {
                warn!(error = %e, "voice.opus_encode_failed");
                break;
            }
        }
    }
    frames
}

/// Returns `false` to terminate the connection (after handshake errors).
async fn handle_control_text(
    text: &str,
    out_tx: &mpsc::Sender<Message>,
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
                    out_tx,
                    "rate_mismatch",
                    &format!("expected capture_rate={OPUS_INPUT_SAMPLE_RATE}"),
                )
                .await;
                return false;
            }
            if playback_rate != TTS_OUTPUT_SAMPLE_RATE {
                send_error(
                    out_tx,
                    "rate_mismatch",
                    &format!("expected playback_rate={TTS_OUTPUT_SAMPLE_RATE}"),
                )
                .await;
                return false;
            }
            if codec != "opus" {
                send_error(out_tx, "unsupported_codec", &format!("expected opus, got {codec}"))
                    .await;
                return false;
            }
            match VoiceSession::new(session_id) {
                Ok(s) => {
                    let turn_id = s.turn_id.clone();
                    *session = Some(s);
                    send_state(out_tx, VoicePhase::Listening, &turn_id).await;
                    true
                }
                Err(e) => {
                    error!(error = %e, "voice.session_init_failed");
                    send_error(out_tx, "session_init_failed", e).await;
                    false
                }
            }
        }
        Ok(ClientMessage::VadHint { .. }) => true,
        Ok(ClientMessage::BargeIn {
            ts_ms,
            playback_ms_played,
        }) => {
            debug!(ts_ms, playback_ms_played, "voice.barge_in_received");
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
            handle_control_action(action, out_tx, session).await;
            true
        }
        Err(e) => {
            warn!(error = %e, "voice.invalid_json");
            send_error(out_tx, "invalid_json", &format!("{e}")).await;
            true
        }
    }
}

async fn handle_control_action(
    action: ControlAction,
    out_tx: &mpsc::Sender<Message>,
    session: &mut Option<VoiceSession>,
) {
    match action {
        ControlAction::Abort => {
            info!("voice.control.abort");
            if let Some(s) = session.as_ref() {
                send_state(out_tx, VoicePhase::Idle, &s.turn_id).await;
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
    let payload = ServerMessage::Error {
        code: code.to_string(),
        message: message.to_string(),
    };
    if let Ok(text) = serde_json::to_string(&payload) {
        drop(tx.send(Message::Text(Utf8Bytes::from(text))).await);
    }
}

async fn send_state(out_tx: &mpsc::Sender<Message>, phase: VoicePhase, turn_id: &str) {
    send_json(
        out_tx,
        &ServerMessage::State {
            phase,
            turn_id: turn_id.to_string(),
        },
    )
    .await;
}

async fn send_error(out_tx: &mpsc::Sender<Message>, code: &str, message: &str) {
    send_json(
        out_tx,
        &ServerMessage::Error {
            code: code.to_string(),
            message: message.to_string(),
        },
    )
    .await;
}

async fn send_json<T: serde::Serialize>(out_tx: &mpsc::Sender<Message>, msg: &T) {
    let Ok(text) = serde_json::to_string(msg) else {
        return;
    };
    drop(out_tx.send(Message::Text(Utf8Bytes::from(text))).await);
}
