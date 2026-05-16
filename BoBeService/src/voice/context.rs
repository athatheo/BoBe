//! Per-WS voice context — bundles the immutable Arc-backed dependencies
//! that `voice.rs`, `voice/control.rs`, and `voice/turn_flow.rs` all need.
//!
//! Eliminates the parameter-drilling smell where every function in the
//! voice subsystem took `&out_tx, &mut session, &engines, &voice_defaults,
//! voice_turn_active`. Now they take `&VoiceContext` instead and reach
//! into the fields they need.
//!
//! Construction site: `voice.rs::handle_socket` after the WS-accept
//! snapshots are computed (engines, defaults, sink install). Lives in
//! the future scope of one WS connection.
//!
//! NOT stored on `AppState` — the per-WS scope owns it.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use axum::extract::ws::Message;
use tokio::sync::mpsc;

use crate::runtime::session::RuntimeSession;
use crate::voice::engines::VoiceEngines;
use crate::voice::session::VoiceDefaults;

/// Bundle of per-WS dependencies shared across the voice handler split.
/// All fields are cheap to clone (Arc or value-type snapshot). The
/// outbound `mpsc::Sender` clones to per-task copies so the writer task
/// stays the sole owner of the WS sink.
#[derive(Clone)]
pub(crate) struct VoiceContext {
    /// Outbound WS sender (cloned per turn task; writer task is sole consumer).
    pub(crate) out_tx: mpsc::Sender<Message>,
    /// Conversation pipeline entry — owned by AppState, Arc-cloned per WS.
    pub(crate) runtime_session: Arc<RuntimeSession>,
    /// Per-WS engines snapshot. Cheap to clone (4 Arc::clones); pass owned
    /// copies into spawned turn tasks so a mid-session install doesn't
    /// yank engines from an in-flight turn.
    pub(crate) engines: VoiceEngines,
    /// Snapshot of voice.* config at WS-accept; per-WS scope, not live.
    pub(crate) voice_defaults: VoiceDefaults,
    /// Voice-turn-active flag. Set by `process_turn`, read by BobeHooks.
    pub(crate) voice_turn_active: Arc<AtomicBool>,
    /// Spawned turn tasks signal here when they exit (natural completion
    /// OR early-return after a `try_begin_user_message` Err). `voice.rs`'s
    /// recv loop drains this and clears `session.current_turn`. Without
    /// this, the slot stays `Some(finished_join)` after a turn and the next
    /// `TranscriptFinal` is dropped at the single-flight gate — voice mode
    /// would lock up after exactly one turn.
    pub(crate) turn_completion_tx: mpsc::Sender<()>,
}
