//! WS endpoint `/voice/stream` (Mode B). Hello handshake → client streams
//! `transcript_partial`/`transcript_final` JSON → daemon admits single-
//! flight + spawns `run_text_turn` → streams TTS Opus binary back.
//! Barge-in or cancel-phrase aborts the active turn; disconnect tears down.

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
use crate::voice::sinks::SinkGuard;

const OUTBOUND_CHANNEL_CAPACITY: usize = 64;
/// Server Ping cadence; auto-Pong from the client doubles as freshness.
const KEEPALIVE_PING_INTERVAL: Duration = Duration::from_secs(25);
/// Recv-side close window; auto-Pongs land inside it for any live client.
const KEEPALIVE_STALE_TIMEOUT: Duration = Duration::from_mins(1);

/// Drop-on-panic clears the flag so a crashed handler doesn't lock the slot.
struct VoiceWsPermit(Arc<AtomicBool>);

impl Drop for VoiceWsPermit {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// Returns `None` after `voice_busy` close so callers can `return` directly.
async fn acquire_permit(socket: WebSocket, state: &AppState) -> Option<(WebSocket, VoiceWsPermit)> {
    if state
        .voice
        .voice_ws_active
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        warn!("voice.ws_concurrent_rejected");
        close_with_error(
            socket,
            "voice_busy",
            "another voice session is already active",
        )
        .await;
        return None;
    }
    Some((
        socket,
        VoiceWsPermit(Arc::clone(&state.voice.voice_ws_active)),
    ))
}

/// Bag passed to `teardown_session` so cleanup drop-order is centralized.
struct SessionTeardown {
    keepalive: tokio::task::JoinHandle<()>,
    session: Option<VoiceSession>,
    writer: tokio::task::JoinHandle<()>,
    sink_guard: SinkGuard,
    ctx: VoiceContext,
    out_tx: mpsc::Sender<Message>,
}

/// Bounded drain; kokoro_task isn't cancellation-aware until synth returns,
/// so without this the ws_permit leaks for 5-15s. Clean drains take <10ms.
const WRITER_DRAIN_TIMEOUT: Duration = Duration::from_secs(2);

async fn teardown_session(state: &Arc<AppState>, td: SessionTeardown) {
    let SessionTeardown {
        keepalive,
        mut session,
        writer,
        sink_guard,
        ctx,
        out_tx,
    } = td;

    // Abort eagerly to suppress a final stray Ping post-disconnect.
    keepalive.abort();

    if let Some(s) = session.as_mut() {
        if let Some(turn) = s.current_turn.take() {
            info!(turn_id = %turn.turn_id, "voice.disconnect_aborting_turn");
            turn.join.abort();
            drop(turn.join.await);
        }
        info!(session = %s.session_id, "voice.disconnect");
    }

    // Sync clear so writer.await sees all senders gone; SinkGuard::drop is panic fallback only.
    state
        .voice
        .voice_sink
        .uninstall_if_current(sink_guard.generation)
        .await;
    drop(sink_guard);

    // Order matters: ctx holds an out_tx clone; drop it first or writer hangs.
    drop(ctx);
    drop(out_tx);

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

/// Sole owner of `ws_tx`; everyone else posts via cloned `out_tx`. Exits on all-senders-dropped.
fn spawn_writer(
    mut ws_tx: futures::stream::SplitSink<WebSocket, Message>,
    mut out_rx: mpsc::Receiver<Message>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            if ws_tx.send(msg).await.is_err() {
                break;
            }
        }
        drop(ws_tx.close().await);
    })
}

/// 25s Pings; recv `KEEPALIVE_STALE_TIMEOUT` catches dead clients via missing Pongs.
fn spawn_keepalive(out_tx: mpsc::Sender<Message>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(KEEPALIVE_PING_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        ticker.tick().await; // skip the immediate first tick
        loop {
            ticker.tick().await;
            if out_tx.send(Message::Ping(Vec::new().into())).await.is_err() {
                break;
            }
        }
    })
}

pub(crate) async fn voice_stream(
    mut ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    // Echo the chosen subprotocol per RFC 6455; unversioned clients still work.
    let selected = ws
        .requested_protocols()
        .find(|p| p.as_bytes() == crate::constants::voice_wire::SUBPROTOCOL_V1.as_bytes())
        .cloned();
    if let Some(p) = selected {
        ws.set_selected_protocol(p);
    }
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

    // Snapshot at accept-time so a mid-session settings hot-swap doesn't retro-change Hello.
    let voice_defaults = VoiceDefaults::from_state(&state);
    if !voice_defaults.enabled {
        warn!("voice.disabled_by_settings");
        close_with_error(socket, "voice_disabled", "voice mode disabled in settings").await;
        return;
    }

    let Some((socket, _ws_permit)) = acquire_permit(socket, &state).await else {
        return;
    };

    let (ws_tx, mut rx) = socket.split();
    let (out_tx, out_rx) = mpsc::channel::<Message>(OUTBOUND_CHANNEL_CAPACITY);

    let writer = spawn_writer(ws_tx, out_rx);
    let keepalive = spawn_keepalive(out_tx.clone());

    let mut session: Option<VoiceSession> = None;
    let runtime_session = Arc::clone(&state.runtime.runtime_session);

    // Install WS sink; SinkGuard::drop on scope-exit makes mid-turn hook fires no-op.
    let sink_guard = state.voice.voice_sink.install(out_tx.clone()).await;

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
        voice_turn_active: Arc::clone(&state.voice.voice_turn_active),
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

    teardown_session(
        &state,
        SessionTeardown {
            keepalive,
            session,
            writer,
            sink_guard,
            ctx,
            out_tx,
        },
    )
    .await;
}
