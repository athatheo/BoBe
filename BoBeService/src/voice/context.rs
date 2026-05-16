//! Per-WS voice dep bundle. Built in `voice.rs::handle_socket` after
//! engines + defaults are snapshotted; threaded through control / turn
//! flow / transcript_in. NOT on AppState — per-WS scope owns it.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use axum::extract::ws::Message;
use tokio::sync::mpsc;

use crate::runtime::session::RuntimeSession;
use crate::voice::engines::VoiceEngines;
use crate::voice::session::VoiceDefaults;

#[derive(Clone)]
pub(crate) struct VoiceContext {
    pub(crate) out_tx: mpsc::Sender<Message>,
    pub(crate) runtime_session: Arc<RuntimeSession>,
    /// Engines snapshot at WS-accept so a mid-session install can't yank
    /// engines from an in-flight turn.
    pub(crate) engines: VoiceEngines,
    pub(crate) voice_defaults: VoiceDefaults,
    pub(crate) voice_turn_active: Arc<AtomicBool>,
    /// Spawned turn tasks signal here on exit so `voice.rs`'s recv loop
    /// can clear `session.current_turn` — without this the next
    /// TranscriptFinal is dropped at the single-flight gate (B01).
    pub(crate) turn_completion_tx: mpsc::Sender<()>,
}
