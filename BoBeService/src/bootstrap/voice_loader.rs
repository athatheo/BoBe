//! Voice engine loader — reads ONNX models from `~/.bobe/models/` and
//! constructs the runtime engine traits. Called at bootstrap and again
//! by VoiceInstallService on install completion (single source of truth).

use std::sync::Arc;

use tracing::{info, warn};

use crate::voice::engines::VoiceEnginesSnapshot;

/// Build a `VoiceEnginesSnapshot` from the models currently on disk.
/// Includes async TTS filler-library synthesis when TTS loads.
pub(super) async fn build_voice_engines_snapshot() -> VoiceEnginesSnapshot {
    let (stt, tts, vad, smart_turn) = load_engine_files();
    let filler_library = match tts.as_ref() {
        Some(tts_engine) => Some(Arc::new(
            crate::voice::filler_library::FillerLibrary::render(Arc::clone(tts_engine)).await,
        )),
        None => None,
    };
    VoiceEnginesSnapshot {
        stt,
        tts,
        vad,
        smart_turn,
        filler_library,
    }
}

/// Best-effort load of the four runtime engines from `~/.bobe/models/`.
/// Sync because every loader is sync (sherpa-onnx + tract inits).
///
/// Provider selection per platform: macOS → CoreML EP, others → CPU.
/// Env overrides: `BOBE_VOICE_PROVIDER` (cpu|coreml|cuda|directml),
/// `BOBE_VOICE_NUM_THREADS` (default 4).
fn load_engine_files() -> (
    Option<Arc<dyn crate::speech::StreamingSttEngine>>,
    Option<Arc<dyn crate::speech::TtsEngine>>,
    Option<Arc<dyn crate::speech::AcousticVad>>,
    Option<Arc<dyn crate::speech::SemanticTurn>>,
) {
    let Some(home) = dirs::home_dir() else {
        warn!("voice.load: no home dir, skipping engine load");
        return (None, None, None, None);
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

    let stt = {
        let dir = models_root.join("sherpa-onnx-streaming-zipformer-en");
        if dir.exists() {
            match crate::speech::streaming_stt::LocalZipformerStt::load(
                &dir, &provider, num_threads,
            ) {
                Ok(e) => {
                    info!(path = %dir.display(), provider, "voice.streaming_stt_loaded");
                    Some(Arc::new(e) as Arc<dyn crate::speech::StreamingSttEngine>)
                }
                Err(e) => {
                    warn!(error = %e, "voice.streaming_stt_load_failed");
                    None
                }
            }
        } else {
            warn!(path = %dir.display(), "voice.stt_model_missing");
            None
        }
    };

    let tts = {
        let dir = models_root.join("kokoro-multi-lang-v1_0");
        let model = dir.join("model.onnx");
        let voices = dir.join("voices.bin");
        if model.exists() && voices.exists() {
            match crate::speech::local_kokoro::LocalKokoroTts::load(
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
        } else {
            warn!(path = %dir.display(), "voice.tts_model_missing");
            None
        }
    };

    let vad = {
        let path = models_root.join("silero-vad").join("silero_vad.onnx");
        if path.exists() {
            match crate::speech::local_silero::LocalSileroVad::load(&path, &provider, num_threads) {
                Ok(e) => {
                    info!(path = %path.display(), provider, "voice.vad_loaded");
                    Some(Arc::new(e) as Arc<dyn crate::speech::AcousticVad>)
                }
                Err(e) => {
                    warn!(error = %e, "voice.vad_load_failed");
                    None
                }
            }
        } else {
            warn!(path = %path.display(), "voice.vad_model_missing");
            None
        }
    };

    // Smart-turn v3.2 — real ONNX gate via tract. Required at boot when
    // any voice engine loaded (per D9). If the model file is missing the
    // user must run `scripts/install-voice-models.sh`; the daemon refuses
    // to start silently-degraded.
    let smart_turn = {
        let path = models_root.join("smart-turn-v3.2-cpu.onnx");
        if path.exists() {
            match crate::speech::smart_turn_onnx::LocalSmartTurnOnnx::load(&path) {
                Ok(e) => {
                    info!(path = %path.display(), "voice.smart_turn_loaded");
                    Some(Arc::new(e) as Arc<dyn crate::speech::SemanticTurn>)
                }
                Err(e) => {
                    warn!(error = %e, "voice.smart_turn_load_failed");
                    None
                }
            }
        } else {
            warn!(path = %path.display(), "voice.smart_turn_model_missing");
            None
        }
    };

    (stt, tts, vad, smart_turn)
}
