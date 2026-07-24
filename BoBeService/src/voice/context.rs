//! Per-endpoint voice dependency bundle. Mac WebSocket and physical-body
//! adapters both create one after snapshotting engines and defaults.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tokio::sync::mpsc;

use crate::runtime::response_streamer::StreamDelivery;
use crate::runtime::session::RuntimeSession;
use crate::voice::engines::VoiceEngines;
use crate::voice::output::VoiceOutput;
use crate::voice::session::VoiceDefaults;
use crate::voice::sinks::VoiceSink;

#[derive(Clone)]
pub(crate) struct VoiceContext {
    pub(crate) output: VoiceOutput,
    pub(crate) runtime_session: Arc<RuntimeSession>,
    /// Engines snapshot at WS-accept so a mid-session install can't yank
    /// engines from an in-flight turn.
    pub(crate) engines: VoiceEngines,
    pub(crate) voice_defaults: VoiceDefaults,
    pub(crate) voice_turn_active: Arc<AtomicBool>,
    pub(crate) voice_sink: Arc<VoiceSink>,
    pub(crate) delivery: StreamDelivery,
    /// Physical bodies need the same sentence text that server TTS speaks so
    /// their display can show a bounded caption. Mac server-TTS does not.
    pub(crate) send_server_tts_text: bool,
    /// Spawned turn tasks signal here on exit so `voice.rs`'s recv loop
    /// can clear `session.current_turn` — without this the next
    /// TranscriptFinal is dropped at the single-flight gate (B01).
    pub(crate) turn_completion_tx: mpsc::Sender<()>,
}
