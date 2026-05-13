//! Snapshot of every voice engine the daemon loads at runtime. Held on
//! AppState behind an `ArcSwap` so the voice install service can hot-swap
//! after a successful download — no daemon restart required.
//!
//! Voice WS handler reads via `state.voice_engines.load()` at connect
//! time; the per-WS scope captures the snapshot Arc so an install
//! completing mid-session doesn't yank engines from an in-flight turn.

use std::sync::Arc;

use crate::speech::{AcousticVad, SemanticTurn, StreamingSttEngine, TtsEngine};
use crate::voice::filler_library::FillerLibrary;

#[derive(Default)]
pub(crate) struct VoiceEnginesSnapshot {
    pub(crate) stt: Option<Arc<dyn StreamingSttEngine>>,
    pub(crate) tts: Option<Arc<dyn TtsEngine>>,
    pub(crate) vad: Option<Arc<dyn AcousticVad>>,
    pub(crate) smart_turn: Option<Arc<dyn SemanticTurn>>,
    pub(crate) filler_library: Option<Arc<FillerLibrary>>,
}

impl VoiceEnginesSnapshot {
    /// True when all four runtime engines are loaded. The filler library
    /// is intentionally not part of this check — voice still works without
    /// it (just silent during gaps).
    pub(crate) fn is_complete(&self) -> bool {
        self.stt.is_some()
            && self.tts.is_some()
            && self.vad.is_some()
            && self.smart_turn.is_some()
    }
}
