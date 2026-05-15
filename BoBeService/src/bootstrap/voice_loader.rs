//! Voice engine loader — reads the Kokoro TTS model from `~/.bobe/models/`
//! and constructs the runtime engine snapshot. Mode B: daemon owns TTS
//! only; STT/VAD/smart-turn moved to the Swift client (FluidAudio).
//! Called at bootstrap and again by `VoiceInstallService` on install
//! completion (single source of truth).

use std::sync::Arc;

use tracing::{info, warn};

use crate::voice::engines::VoiceEnginesSnapshot;

/// Build a `VoiceEnginesSnapshot` from the models currently on disk.
/// Includes async TTS filler-library synthesis when TTS loads.
pub(super) async fn build_voice_engines_snapshot() -> VoiceEnginesSnapshot {
    let tts = load_tts();
    let filler_library = match tts.as_ref() {
        Some(tts_engine) => Some(Arc::new(
            crate::voice::filler_library::FillerLibrary::render(Arc::clone(tts_engine)).await,
        )),
        None => None,
    };
    VoiceEnginesSnapshot {
        tts,
        filler_library,
    }
}

/// Best-effort load of Kokoro TTS from `~/.bobe/models/`. Sync because the
/// loader is sync (sherpa-onnx init).
///
/// Provider selection per platform: macOS → CoreML EP, others → CPU.
/// Env overrides: `BOBE_VOICE_PROVIDER` (cpu|coreml|cuda|directml),
/// `BOBE_VOICE_NUM_THREADS` (default 4).
fn load_tts() -> Option<Arc<dyn crate::speech::TtsEngine>> {
    let Some(home) = dirs::home_dir() else {
        warn!("voice.load: no home dir, skipping engine load");
        return None;
    };
    let models_root = home.join(".bobe").join("models");

    let provider = std::env::var("BOBE_VOICE_PROVIDER").unwrap_or_else(|_| {
        if cfg!(target_os = "macos") {
            "coreml".to_string()
        } else {
            "cpu".to_string()
        }
    });
    let num_threads: i32 = std::env::var("BOBE_VOICE_NUM_THREADS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(4);

    let dir = models_root.join("kokoro-multi-lang-v1_0");
    let model = dir.join("model.onnx");
    let voices = dir.join("voices.bin");
    if !model.exists() || !voices.exists() {
        warn!(path = %dir.display(), "voice.tts_model_missing");
        return None;
    }
    match crate::speech::providers::sherpa::kokoro_tts::LocalKokoroTts::load(
        &model,
        &voices,
        &provider,
        num_threads,
    ) {
        Ok(e) => {
            info!(path = %dir.display(), provider, "voice.tts_loaded");
            Some(Arc::new(e) as Arc<dyn crate::speech::TtsEngine>)
        }
        Err(e) => {
            warn!(error = %e, "voice.tts_load_failed");
            None
        }
    }
}
