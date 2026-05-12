//! WS endpoint `/voice/stream`.
//!
//! Lifecycle per session:
//!   - WS upgrade + handshake (rate + codec validation)
//!   - Audio decode → Silero VAD per-frame → speech-segment queue
//!   - On segment: smart-turn gate → spawn a per-turn task (`process_turn`)
//!     that runs STT + chat pipeline + Kokoro TTS in the background while
//!     the WS rx loop keeps reading control messages.
//!   - Client BargeIn → main loop aborts the current turn JoinHandle →
//!     UserMessageGuard drops + AbortGuard on the SDK stream fires
//!     `session.abort()` → Truncate + state(Listening) sent to client.
//!   - Disconnect → abort any in-flight turn before tearing down.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use axum::extract::State;
use axum::extract::ws::{Message, Utf8Bytes, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};
use opus::{Application, Channels, Decoder as OpusDecoder, Encoder as OpusEncoder};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::runtime::session::RuntimeSession;
use crate::speech::markdown_strip::MarkdownStripper;
use crate::speech::protocol::{
    encode_tts_frame, ClientMessage, ControlAction, ServerMessage, VoicePhase, FLAG_FILLER,
    FLAG_FIRST_OF_TURN,
};
use crate::speech::sentence_buffer::SentenceBuffer;
use crate::speech::{AcousticVad, SemanticTurn, SttEngine, TtsEngine};

const OPUS_INPUT_SAMPLE_RATE: u32 = 16_000;
const TTS_OUTPUT_SAMPLE_RATE: u32 = 24_000;
const OPUS_MAX_FRAME_SAMPLES: usize = 2_880;
const TTS_OPUS_BITRATE_BPS: i32 = 24_000;
const OUTBOUND_CHANNEL_CAPACITY: usize = 64;
/// Sentence buffer between observer (synchronous closure) and Kokoro task.
/// Bursts during fast LLM token rates can push 3-5 sentences within ~200ms;
/// 16 gives comfortable headroom while still bounding memory.
const SENTENCE_CHANNEL_CAPACITY: usize = 16;

/// Smart-turn pass-through threshold. Stub returns 1.0 → trivially passes;
/// real smart-turn v3.1 (M4.5.2) will gate here.
const TURN_COMPLETE_THRESHOLD: f32 = 0.7;

/// Time-to-first-audio budget. If the Kokoro task hasn't pushed any audio
/// frames by this deadline (the LLM is slow or the first sentence is still
/// buffering), the filler watchdog emits the pre-rendered cached phrase.
/// Matches LiveKit/Pipecat/ElevenLabs production threshold.
const FILLER_TRIGGER: Duration = Duration::from_millis(800);

/// Default voice slot — replaced by per-soul `voice_id` setting in M4.5.0c.
const DEFAULT_KOKORO_VOICE: &str = "af_bella";
const DEFAULT_KOKORO_SPEED: f32 = 1.0;

pub(crate) async fn voice_stream(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Engines fanned out per WS connection. `Clone` is cheap (4 `Arc::clone`s)
/// so we can pass an owned copy into each spawned turn task.
#[derive(Clone)]
struct VoiceEngines {
    stt: Arc<dyn SttEngine>,
    tts: Arc<dyn TtsEngine>,
    vad: Arc<dyn AcousticVad>,
    smart_turn: Arc<dyn SemanticTurn>,
    /// Pre-rendered filler PCM, populated at bootstrap. `None` when TTS
    /// failed to render it; turn still works, just silent during the
    /// LLM-think gap.
    filler_pcm: Option<Arc<Vec<f32>>>,
}

impl VoiceEngines {
    fn from_state(state: &AppState) -> Option<Self> {
        Some(Self {
            stt: Arc::clone(state.voice_stt.as_ref()?),
            tts: Arc::clone(state.voice_tts.as_ref()?),
            vad: Arc::clone(state.voice_vad.as_ref()?),
            smart_turn: Arc::clone(state.voice_smart_turn.as_ref()?),
            filler_pcm: state.voice_filler_pcm.as_ref().map(Arc::clone),
        })
    }
}

/// Per-WS state — one struct, lives in the handle_socket future scope.
struct VoiceSession {
    session_id: String,
    decoder: OpusDecoder,
    /// JoinHandle on the spawned `process_turn` task plus the turn_id it owns.
    /// `None` outside a turn; populated when audio commits, taken when a
    /// barge-in or natural completion releases it.
    current_turn: Option<TurnInFlight>,
}

struct TurnInFlight {
    turn_id: String,
    join: JoinHandle<()>,
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

    // Dedicated WS writer task — sole owner of ws_tx. Main loop, per-turn
    // tasks, and the Kokoro task all post via cloned out_tx senders.
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
                // Reap a finished turn before processing the next segment.
                reap_finished_turn(s);
                while engines.vad.has_segment() {
                    if let Some(segment) = engines.vad.pop_segment() {
                        if s.current_turn.is_some() {
                            // A turn is already running. Drop the segment for
                            // now — M5.2 hammering pushback will queue these
                            // and merge into the in-flight or next turn.
                            debug!("voice.segment_dropped_turn_in_flight");
                            continue;
                        }
                        let turn = spawn_turn(
                            segment,
                            engines.clone(),
                            Arc::clone(&runtime_session),
                            out_tx.clone(),
                        );
                        s.current_turn = Some(turn);
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

    // Clean up any in-flight turn before tearing down the socket.
    if let Some(mut s) = session {
        if let Some(turn) = s.current_turn.take() {
            info!(turn_id = %turn.turn_id, "voice.disconnect_aborting_turn");
            turn.join.abort();
            drop(turn.join.await);
        }
        info!(session = %s.session_id, "voice.disconnect");
    }
    drop(out_tx);
    match writer.await {
        Ok(()) => {}
        Err(e) if e.is_panic() => {
            error!(error = %e, "voice.ws_writer_panic");
        }
        Err(e) if e.is_cancelled() => {}
        Err(e) => warn!(error = %e, "voice.ws_writer_join_failed"),
    }
}

/// If the current turn has completed naturally, clear the handle so a fresh
/// turn can be admitted. Called opportunistically before each new segment.
fn reap_finished_turn(session: &mut VoiceSession) {
    if let Some(turn) = session.current_turn.as_ref()
        && turn.join.is_finished()
    {
        session.current_turn = None;
    }
}

fn spawn_turn(
    segment: crate::speech::vad::SpeechSegment,
    engines: VoiceEngines,
    runtime_session: Arc<RuntimeSession>,
    out_tx: mpsc::Sender<Message>,
) -> TurnInFlight {
    let turn_id = format!("voice_{}", Uuid::new_v4().simple());
    let task_turn_id = turn_id.clone();
    let join = tokio::spawn(async move {
        process_turn(segment, engines, runtime_session, out_tx, task_turn_id).await;
    });
    TurnInFlight { turn_id, join }
}

#[tracing::instrument(name = "voice.turn", skip_all, fields(turn_id = %turn_id, samples = segment.samples.len()))]
async fn process_turn(
    segment: crate::speech::vad::SpeechSegment,
    engines: VoiceEngines,
    runtime_session: Arc<RuntimeSession>,
    out_tx: mpsc::Sender<Message>,
    turn_id: String,
) {
    let turn_start = Instant::now();

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
            send_error(&out_tx, "conflict", reason).await;
            return;
        }
    };

    send_state(&out_tx, VoicePhase::Thinking, &turn_id).await;

    // STT
    let stt = Arc::clone(&engines.stt);
    let samples = segment.samples;
    let stt_start = Instant::now();
    let stt_result = tokio::task::spawn_blocking(move || stt.transcribe(&samples)).await;
    let transcript = match stt_result {
        Ok(Ok(text)) => text,
        Ok(Err(e)) => {
            error!(error = %e, "voice.stt_failed");
            send_error(&out_tx, "stt_failed", &e.to_string()).await;
            send_state(&out_tx, VoicePhase::Listening, &turn_id).await;
            drop(guard);
            return;
        }
        Err(e) => {
            error!(error = %e, "voice.stt_join_failed");
            send_error(&out_tx, "stt_join_failed", &e.to_string()).await;
            send_state(&out_tx, VoicePhase::Listening, &turn_id).await;
            drop(guard);
            return;
        }
    };

    let stt_elapsed_ms = stt_start.elapsed().as_millis() as u64;
    let trimmed = transcript.trim();
    if trimmed.is_empty() {
        info!(stt_ms = stt_elapsed_ms, "voice.empty_transcript");
        send_error(&out_tx, "empty_transcript", "I didn't catch that").await;
        send_state(&out_tx, VoicePhase::Listening, &turn_id).await;
        drop(guard);
        return;
    }
    info!(
        stt_ms = stt_elapsed_ms,
        chars = trimmed.len(),
        transcript = %trimmed,
        "voice.transcript_final"
    );
    send_json(
        &out_tx,
        &ServerMessage::TranscriptFinal {
            turn_id: turn_id.clone(),
            text: trimmed.to_string(),
        },
    )
    .await;

    // Spin up the per-turn TTS pipeline
    send_state(&out_tx, VoicePhase::Speaking, &turn_id).await;
    let speaking_start = Instant::now();
    let first_audio_emitted = Arc::new(AtomicBool::new(false));
    let filler_task = spawn_filler_watchdog(
        engines.filler_pcm.as_ref().map(Arc::clone),
        engines.tts.sample_rate(),
        Arc::clone(&first_audio_emitted),
        out_tx.clone(),
    );
    let (sentence_tx, sentence_rx) = mpsc::channel::<String>(SENTENCE_CHANNEL_CAPACITY);
    let kokoro_task = spawn_kokoro_task(
        Arc::clone(&engines.tts),
        sentence_rx,
        out_tx.clone(),
        Arc::clone(&first_audio_emitted),
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

    runtime_session
        .handle_user_message_with_observer(trimmed, &turn_id, observer)
        .await;

    // Flush trailing sentence (LLM may end without trailing punctuation)
    if let Ok(mut p) = pipeline.lock() {
        p.flush();
    }
    drop(pipeline);
    drop(sentence_tx);
    match kokoro_task.await {
        Ok(()) => {}
        Err(e) if e.is_panic() => {
            error!(error = %e, "voice.kokoro_task_panic");
        }
        Err(e) if e.is_cancelled() => {
            debug!("voice.kokoro_task_cancelled");
        }
        Err(e) => warn!(error = %e, "voice.kokoro_task_join_failed"),
    }

    // Cancel the filler watchdog if it hasn't fired yet (LLM was fast enough).
    if let Some(handle) = filler_task {
        handle.abort();
        drop(handle.await);
    }

    send_json(
        &out_tx,
        &ServerMessage::TtsEnd {
            turn_id: turn_id.clone(),
        },
    )
    .await;
    send_state(&out_tx, VoicePhase::Listening, &turn_id).await;
    drop(guard);

    info!(
        stt_ms = stt_elapsed_ms,
        speaking_ms = speaking_start.elapsed().as_millis() as u64,
        total_ms = turn_start.elapsed().as_millis() as u64,
        "voice.turn_complete"
    );
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
            if self.tx.try_send(sentence).is_err() {
                warn!("voice.sentence_channel_full_drop");
            }
        }
    }

    fn flush(&mut self) {
        for sentence in self.sb.flush() {
            if self.tx.try_send(sentence).is_err() {
                warn!("voice.sentence_channel_full_flush_drop");
            }
        }
    }
}

fn spawn_kokoro_task(
    tts: Arc<dyn TtsEngine>,
    mut sentence_rx: mpsc::Receiver<String>,
    out_tx: mpsc::Sender<Message>,
    first_audio_emitted: Arc<AtomicBool>,
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
                    // Claim the "first audio emitted" slot so the filler
                    // watchdog skips even if it's about to fire.
                    first_audio_emitted.swap(true, Ordering::AcqRel);
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

/// 800ms TTFT filler watchdog. Sleeps then races the Kokoro task for the
/// "first audio emitted" flag. If we win (Kokoro hasn't produced anything
/// yet), encode the pre-rendered filler PCM as Opus 20ms frames and stream
/// them with `FLAG_FILLER` set so the client knows they're preemptible.
/// Returns `None` when there's no filler cache available (graceful no-op).
fn spawn_filler_watchdog(
    filler_pcm: Option<Arc<Vec<f32>>>,
    sample_rate: u32,
    first_audio_emitted: Arc<AtomicBool>,
    out_tx: mpsc::Sender<Message>,
) -> Option<JoinHandle<()>> {
    let pcm = filler_pcm?;
    Some(tokio::spawn(async move {
        tokio::time::sleep(FILLER_TRIGGER).await;
        // Atomic claim — if Kokoro already emitted, swap returns true and we exit.
        if first_audio_emitted.swap(true, Ordering::AcqRel) {
            debug!("voice.filler_skipped_kokoro_already_emitted");
            return;
        }
        debug!(samples = pcm.len(), "voice.filler_emit");
        let mut chunk_id: u64 = 0;
        for opus_packet in encode_opus_frames(&pcm, sample_rate) {
            let framed = encode_tts_frame(chunk_id, FLAG_FILLER, &opus_packet);
            chunk_id = chunk_id.saturating_add(1);
            if out_tx.send(Message::Binary(framed.into())).await.is_err() {
                return;
            }
        }
    }))
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
                    let initial_turn = format!("voice_{}", Uuid::new_v4().simple());
                    *session = Some(s);
                    send_state(out_tx, VoicePhase::Listening, &initial_turn).await;
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
            info!(ts_ms, playback_ms_played, "voice.barge_in_received");
            handle_barge_in(out_tx, session, playback_ms_played).await;
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

/// 3-event barge-in: (1) abort the in-flight turn task — its drop fires the
/// SDK AbortGuard which calls `session.abort()` and cascades to the Kokoro
/// task via the dropped sentence channel; (2) send `truncate` so the client
/// drops queued audio past `played_ms`; (3) send `state(Listening)` so the
/// mic UI clears immediately. Daemon awaits the task's natural unwind (which
/// can take up to one spawn_blocking call, since spawn_blocking isn't
/// cooperatively cancellable) to ensure the UserMessageGuard releases before
/// the next turn can begin.
async fn handle_barge_in(
    out_tx: &mpsc::Sender<Message>,
    session: &mut Option<VoiceSession>,
    played_ms: u64,
) {
    let Some(s) = session.as_mut() else { return };
    let Some(turn) = s.current_turn.take() else {
        debug!("voice.barge_in_no_turn");
        return;
    };
    info!(turn_id = %turn.turn_id, played_ms, "voice.barge_in_aborting");
    let TurnInFlight { turn_id, join } = turn;
    join.abort();
    // Await drop so UserMessageGuard releases before the next turn can begin.
    drop(join.await);
    send_json(
        out_tx,
        &ServerMessage::Truncate {
            turn_id: turn_id.clone(),
            keep_ms: played_ms,
        },
    )
    .await;
    send_state(out_tx, VoicePhase::Listening, &turn_id).await;
}

async fn handle_control_action(
    action: ControlAction,
    out_tx: &mpsc::Sender<Message>,
    session: &mut Option<VoiceSession>,
) {
    match action {
        ControlAction::Abort => {
            info!("voice.control.abort");
            // Treat explicit abort like a barge-in with played_ms=0.
            handle_barge_in(out_tx, session, 0).await;
            if let Some(s) = session.as_ref() {
                send_state(out_tx, VoicePhase::Idle, &s.session_id).await;
            }
        }
        ControlAction::Mute | ControlAction::Unmute | ControlAction::Reset => {
            debug!(?action, "voice.control_ignored_shell");
        }
    }
}

impl VoiceSession {
    fn new(session_id: String) -> Result<Self, &'static str> {
        let decoder = OpusDecoder::new(OPUS_INPUT_SAMPLE_RATE, Channels::Mono)
            .map_err(|_| "create Opus decoder failed")?;
        Ok(Self {
            session_id,
            decoder,
            current_turn: None,
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
