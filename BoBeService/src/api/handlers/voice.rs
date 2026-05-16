//! WS endpoint `/voice/stream` (Mode B only).
//!
//! Lifecycle per session:
//!   - WS upgrade + Hello handshake (playback_rate validation)
//!   - Client streams `transcript_partial` / `transcript_final` over JSON.
//!     `transcript_final` admits a turn (single-flight) and spawns the
//!     convergence pipeline (`voice/run_text_turn`).
//!   - Daemon streams TTS Opus binary back to the client.
//!   - Barge-in (RMS-detected client-side, sent as `barge_in` control)
//!     or cancel-phrase (regex on echoed partials) → abort the active turn.
//!   - Disconnect → abort any in-flight turn before tearing down.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::app_state::AppState;
use crate::voice::context::VoiceContext;
use crate::voice::control::handle_control_text;
use crate::voice::engines::VoiceEngines;
use crate::voice::protocol_helpers::close_with_error;
use crate::voice::session::{VoiceDefaults, VoiceSession};

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
        let err = crate::speech::protocol::ServerMessage::Error {
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
    // Bundle per-WS deps so subsystem fns don't drill 5 args each.
    // Completion back-channel: spawned per-turn tasks signal here on exit
    // so the recv loop clears `session.current_turn`. Capacity > 1 lets a
    // double-Hello rehello's aborted task and the new turn both signal
    // without blocking. See `voice/modes/transcript_in.rs::spawn_text_turn`.
    let (turn_completion_tx, mut turn_completion_rx) = mpsc::channel::<()>(8);

    let ctx = VoiceContext {
        out_tx: out_tx.clone(),
        runtime_session: Arc::clone(&runtime_session),
        engines: engines.clone(),
        voice_defaults: voice_defaults.clone(),
        voice_turn_active: Arc::clone(&state.voice_turn_active),
        turn_completion_tx,
    };

    loop {
        tokio::select! {
            biased;
            // Reap any naturally-completed turn so the next TranscriptFinal
            // isn't dropped at the single-flight gate. `take()` may return
            // None if abort_active_turn already cleared the slot — safe.
            Some(()) = turn_completion_rx.recv() => {
                if let Some(s) = session.as_mut()
                    && let Some(turn) = s.current_turn.take() {
                    debug!(turn_id = %turn.turn_id, "voice.turn_completed_slot_cleared");
                    // join is already finished; await is instant.
                    drop(turn.join.await);
                }
            }
            next = tokio::time::timeout(KEEPALIVE_STALE_TIMEOUT, rx.next()) => {
                let next = match next {
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
                    Message::Binary(_) => {
                        // Mode B: client never sends audio over the wire. Drop the
                        // frame defensively; well-behaved clients won't trigger this.
                        warn!("voice.unexpected_binary_dropped");
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
    // Bound the writer drain. Per-turn sub-tasks (kokoro_task, filler_task)
    // hold cloned out_tx senders; if the WS disconnected mid-TTS, those
    // tasks may still be running (kokoro inside spawn_blocking is not
    // cancellation-aware until synthesis returns). Without a timeout the
    // writer.await would hold the function open for 5-15s, leaking the
    // ws_permit and voice_busy-rejecting subsequent connections in that
    // window. The 2s bound is a backstop — clean disconnects (no in-flight
    // turn) drain in <10ms.
    const WRITER_DRAIN_TIMEOUT: Duration = Duration::from_secs(2);
    match tokio::time::timeout(WRITER_DRAIN_TIMEOUT, writer).await {
        Ok(Ok(())) => {}
        Ok(Err(e)) if e.is_panic() => {
            error!(error = %e, "voice.ws_writer_panic");
        }
        Ok(Err(e)) if e.is_cancelled() => {}
        Ok(Err(e)) => warn!(error = %e, "voice.ws_writer_join_failed"),
        Err(_) => warn!("voice.ws_writer_drain_timeout_abandoning"),
    }
}
