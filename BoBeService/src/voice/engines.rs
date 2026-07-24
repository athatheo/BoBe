//! Snapshot of the daemon's TTS engine + filler library, held on AppState
//! behind an `ArcSwap` so the voice install service can hot-swap after a
//! successful Kokoro download.
//!
//! Mode B (no daemon-side ASR): only TTS and the pre-rendered fillers
//! remain. Client owns STT/VAD/smart-turn.

use std::sync::Arc;

use crate::app_state::AppState;
use crate::speech::TtsEngine;
use crate::voice::filler_library::FillerLibrary;

#[derive(Default)]
pub(crate) struct VoiceEnginesSnapshot {
    pub(crate) tts: Option<Arc<dyn TtsEngine>>,
    pub(crate) filler_library: Option<Arc<FillerLibrary>>,
}

impl VoiceEnginesSnapshot {
    /// True when TTS is loaded — the only daemon-side engine that gates
    /// voice readiness. Fillers are best-effort (silent gaps acceptable).
    pub(crate) fn is_complete(&self) -> bool {
        self.tts.is_some()
    }
}

/// Engines fanned out per WS connection. Cheap to clone — pass an owned
/// copy into each spawned turn task. Captured at connect time so mid-session
/// installs don't yank engines from an in-flight turn.
#[derive(Clone)]
pub(crate) struct VoiceEngines {
    /// Optional because client-side Supertonic sessions do not require Kokoro.
    pub(crate) tts: Option<Arc<dyn TtsEngine>>,
    /// Pre-rendered filler library, populated at bootstrap. `None` when TTS
    /// engine isn't loaded; turn still works, just silent during gaps.
    pub(crate) fillers: Option<Arc<FillerLibrary>>,
}

impl VoiceEngines {
    pub(crate) fn from_state(state: &AppState) -> Self {
        Self::from_snapshot(&state.voice.voice_engines.load())
    }

    pub(crate) fn from_snapshot(snap: &VoiceEnginesSnapshot) -> Self {
        Self {
            tts: snap.tts.as_ref().map(Arc::clone),
            fillers: snap.filler_library.as_ref().map(Arc::clone),
        }
    }

    pub(crate) fn supports_server_tts(&self) -> bool {
        self.tts.is_some()
    }
}
