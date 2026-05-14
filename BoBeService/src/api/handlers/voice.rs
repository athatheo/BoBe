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
use std::time::Duration;

use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::app_state::AppState;
use crate::speech::protocol::ServerMessage;
use crate::voice::cancel_phrases::is_cancel_phrase;
use crate::voice::context::VoiceContext;
use crate::voice::control::handle_control_text;
use crate::voice::engines::VoiceEngines;
use crate::voice::protocol_helpers::{close_with_error, send_json};
use crate::voice::session::{VoiceDefaults, VoiceSession};
use crate::voice::telemetry::{CTR_CANCEL_PHRASE, CTR_SEGMENT_DROP};
use crate::voice::turn_flow::{abort_active_turn, reap_finished_turn, spawn_turn};

const OUTBOUND_CHANNEL_CAPACITY: usize = 64;
/// Server-initiated Ping cadence. The WS layer auto-responds to Pings with
/// Pongs, so this also doubles as the client's freshness signal.
const KEEPALIVE_PING_INTERVAL: Duration = Duration::from_secs(25);
/// Recv timeout — close the socket if nothing arrives for this long. The
/// 25s server pings trigger auto-Pong from any live client, so a healthy
/// connection always replenishes within this window even when muted.
const KEEPALIVE_STALE_TIMEOUT: Duration = Duration::from_mins(1);

/// RAII guard for the single-flight voice WS permit. Clears the flag on
/// drop including panic-unwind so a crashed handler doesn't lock the slot.
struct VoiceWsPermit(Arc<AtomicBool>);

impl Drop for VoiceWsPermit {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}


pub(crate) async fn voice_stream(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
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

    // Snapshot voice defaults at WS-accept. Settings hot-swap during the
    // connection won't retroactively change in-flight Hello defaults — a
    // new connection picks up the latest config.
    let voice_defaults = VoiceDefaults::from_state(&state);
    if !voice_defaults.enabled {
        warn!("voice.disabled_by_settings");
        close_with_error(socket, "voice_disabled", "voice mode disabled in settings").await;
        return;
    }

    let (ws_tx, mut rx) = socket.split();
    let (out_tx, mut out_rx) = mpsc::channel::<Message>(OUTBOUND_CHANNEL_CAPACITY);

    // Single-flight: voice engines (Silero VAD, streaming Zipformer STT)
    // are Arc-shared globally and not safe for concurrent feed. CAS
    // false→true to acquire; reject with conflict if another connection
    // owns the slot. Cleared via RAII guard on drop.
    if state
        .voice_ws_active
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        warn!("voice.ws_concurrent_rejected");
        let err = ServerMessage::Error {
            code: "voice_busy".into(),
            message: "another voice session is already active".into(),
        };
        if let Ok(json) = serde_json::to_string(&err) {
            drop(out_tx.send(Message::Text(json.into())).await);
        }
        // Drain so writer task exits.
        drop(out_tx);
        let mut ws_tx = ws_tx;
        while let Some(msg) = out_rx.recv().await {
            drop(ws_tx.send(msg).await);
        }
        drop(ws_tx.close().await);
        return;
    }
    let _ws_permit = VoiceWsPermit(Arc::clone(&state.voice_ws_active));

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

    // Keepalive: pings on a 25s cadence. The WS layer auto-responds with
    // Pong, so even a silent (muted) client refreshes the recv-timeout
    // window. Recv timeout below catches the dead-connection case.
    let out_tx_for_ping = out_tx.clone();
    let keepalive = tokio::spawn(async move {
        let mut ticker = tokio::time::interval(KEEPALIVE_PING_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        ticker.tick().await; // skip the immediate first tick
        loop {
            ticker.tick().await;
            if out_tx_for_ping
                .send(Message::Ping(Vec::new().into()))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    let mut session: Option<VoiceSession> = None;
    let runtime_session = Arc::clone(&state.runtime_session);

    // Install the WS sink for the AppState's single-slot voice sink. The
    // returned guard clears the slot on drop (this scope's end), so a
    // mid-turn disconnect lets subsequent hook fires safely no-op.
    let sink_guard = state.voice_sink.install(out_tx.clone()).await;

    // CRITICAL: reset shared engines on WS-accept so prior session state
    // doesn't leak forward. The streaming Zipformer + Silero engines are
    // Arc-shared across all WS handlers via AppState.voice_engines, so a
    // disconnect that happened mid-utterance leaves accumulated decoder
    // state that the next connection's commit_final would surface as
    // "your prior session's text + this session's text" pollution.
    engines.stt.reset();
    engines.vad.reset();

    // Bundle per-WS deps so subsystem fns don't drill 5 args each.
    let ctx = VoiceContext {
        out_tx: out_tx.clone(),
        runtime_session: Arc::clone(&runtime_session),
        engines: engines.clone(),
        voice_defaults: voice_defaults.clone(),
        voice_turn_active: Arc::clone(&state.voice_turn_active),
    };

    loop {
        let next = match tokio::time::timeout(KEEPALIVE_STALE_TIMEOUT, rx.next()).await {
            Ok(Some(m)) => m,
            Ok(None) => break, // stream ended
            Err(_) => {
                warn!("voice.keepalive_recv_timeout_closing");
                break;
            }
        };
        let msg = match next {
            Ok(m) => m,
            Err(e) => {
                warn!(error = %e, "voice.ws_recv_error");
                break;
            }
        };

        match msg {
            Message::Text(text) => {
                if !handle_control_text(text.as_str(), &ctx, &mut session).await {
                    break;
                }
            }
            Message::Binary(bytes) => {
                let Some(s) = session.as_mut() else {
                    warn!("voice.binary_before_hello");
                    continue;
                };
                if s.muted {
                    // Mute gate — drop the frame before VAD ever sees it.
                    continue;
                }
                let samples = match s.decode_opus(&bytes) {
                    Ok(samples) => samples,
                    Err(e) => {
                        warn!(error = %e, "voice.opus_decode_failed");
                        continue;
                    }
                };
                // Feed streaming Zipformer per-frame; emit any new partial.
                // pop_partial dedupes so identical text doesn't get re-sent.
                if let Err(e) = engines.stt.accept_audio(&samples) {
                    warn!(error = %e, "voice.streaming_stt_accept_failed");
                }
                if let Some(partial) = engines.stt.pop_partial() {
                    let turn_id = s
                        .current_turn
                        .as_ref()
                        .map_or_else(|| s.session_id.clone(), |t| t.turn_id.clone());
                    s.last_partial_text.clone_from(&partial);
                    send_json(
                        &out_tx,
                        &ServerMessage::TranscriptPartial {
                            turn_id: turn_id.clone(),
                            text: partial.clone(),
                        },
                    )
                    .await;
                    // C7: cancel-phrase bypass. If the user says "stop" /
                    // "nevermind" during BoBe's TTS, abort the in-flight
                    // turn locally — no LLM round-trip. Bypasses MinWords
                    // because the cancel command IS the signal.
                    if s.current_turn.is_some() && is_cancel_phrase(&partial) {
                        metrics::counter!(CTR_CANCEL_PHRASE).increment(1);
                        info!(partial = %partial, "voice.cancel_phrase_abort");
                        let keep_ms = s.last_acked_played_ms;
                        abort_active_turn(s, &ctx, keep_ms, "cancel_phrase").await;
                        continue;
                    }
                }
                if let Err(e) = engines.vad.accept(&samples) {
                    warn!(error = %e, "voice.vad_accept_failed");
                    continue;
                }
                // Reap a finished turn before processing the next segment.
                reap_finished_turn(s);
                // Drain every COMPLETED segment from the VAD queue. Using
                // `while let Some(...) = pop_segment()` is critical — the
                // earlier `while has_segment() { if let Some(...) = pop... }`
                // pattern spun the worker thread at 100% CPU because
                // `has_segment()` reflects the live "speech detected"
                // signal, not "completed segment in queue", so it stayed
                // true while pop_segment() returned None mid-utterance.
                while let Some(segment) = engines.vad.pop_segment() {
                    if s.current_turn.is_some() {
                        // A turn is already running. Drop the segment for
                        // now — M5.2 hammering pushback will queue these
                        // and merge into the in-flight or next turn.
                        s.segments_dropped =
                            s.segments_dropped.saturating_add(1);
                        metrics::counter!(CTR_SEGMENT_DROP).increment(1);
                        warn!(
                            session = %s.session_id,
                            drops = s.segments_dropped,
                            "voice.segment_backpressure_drop"
                        );
                        continue;
                    }
                    let turn = spawn_turn(segment, &ctx, s.voice_cfg.clone());
                    s.current_turn = Some(turn);
                    // Partial-text carries until the next utterance
                    // begins. Clear here so the MinWords gate doesn't
                    // see stale words from the previous turn.
                    s.last_partial_text.clear();
                }
            }
            Message::Close(_) => {
                info!("voice.close_received");
                break;
            }
            Message::Pong(_) | Message::Ping(_) => {
                // Pong arrives in response to our keepalive Pings (auto-echoed
                // by the WS layer on the client). Pong receipt is implicit
                // keepalive — rx.next() returning at all resets the timeout.
            }
        }
    }

    // Stop the keepalive task; it'll exit naturally when out_tx is dropped
    // below, but aborting first avoids one stray Ping post-disconnect.
    keepalive.abort();

    // Clean up any in-flight turn before tearing down the socket.
    if let Some(mut s) = session {
        if let Some(turn) = s.current_turn.take() {
            info!(turn_id = %turn.turn_id, "voice.disconnect_aborting_turn");
            turn.join.abort();
            drop(turn.join.await);
        }
        info!(session = %s.session_id, "voice.disconnect");
    }
    // Synchronously clear the sink slot so writer.await sees all senders
    // dropped. The Drop on sink_guard at function end is a panic-unwind
    // fallback only — without this explicit await the spawned-task clear
    // races with handle_socket return and the writer hangs.
    state
        .voice_sink
        .uninstall_if_current(sink_guard.generation)
        .await;
    drop(sink_guard);
    // VoiceContext holds an out_tx clone. Drop it BEFORE the explicit
    // out_tx drop below — otherwise out_rx would still have a sender
    // (via ctx.out_tx) and the writer task would hang forever, leaking
    // the _ws_permit and locking the single-flight slot.
    drop(ctx);
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




