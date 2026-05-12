mod database;
mod infra;
mod repos;
mod wiring;

use std::sync::Arc;

use tracing::{info, warn};

use crate::app_state::AppState;
use crate::config::Config;
use crate::error::AppError;

pub(crate) async fn run(config: Config) -> Result<Arc<AppState>, AppError> {
    let pool = database::connect_and_apply_schema(&config.database.url).await?;

    let infra = infra::Infrastructure::build(&config)?;
    let repos = repos::Repositories::from_pool(&pool);

    if config.mcp.enabled
        && let Err(e) =
            crate::mcp::config::ensure_mcp_config_exists(config.mcp.config_file.as_deref())
    {
        warn!(error = %e, "bootstrap.ensure_mcp_config_failed");
    }

    let memory_file = {
        let path = crate::util::paths::bobe_data_dir().join("memory.md");
        crate::copilot::memory_file::MemoryFile::new(path)
    };
    // Idempotent: existing skills are never overwritten.
    crate::copilot::skills::ensure_skills(&crate::util::paths::bobe_data_dir()).await;
    // SDK owns MCP process spawn + tool dispatch via `SessionConfig::mcp_servers`.
    let mcp_servers = load_mcp_servers_for_sdk(&config);
    // Voice-turn signal lives here so AppState (consumed by voice.rs) and
    // WorkerRegistry (consumed by BobeHooks) both reference the same Arc.
    let voice_turn_active = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let workers = {
        let data_dir = crate::util::paths::bobe_data_dir();
        crate::copilot::registry::WorkerRegistry::new(
            Arc::clone(&infra.config_arc),
            Arc::clone(&memory_file),
            data_dir,
            mcp_servers,
            Arc::clone(&voice_turn_active),
        )
    };

    workers.prune_old_chat_sessions().await;

    let wired = wiring::wire(&config, &infra, &repos, Arc::clone(&workers)).await;

    // Engine-config changes trigger registry reload; no daemon restart needed.
    {
        let registry_for_listener = Arc::clone(&workers);
        wired.config_manager.set_engine_change_listener(move || {
            let registry = Arc::clone(&registry_for_listener);
            tokio::spawn(async move {
                registry.reload().await;
            });
        });
    }

    if config.seed_default_documents {
        if let Err(e) = crate::db::seeding::seed_default_souls(repos.soul_repo.as_ref()).await {
            tracing::warn!(error = %e, "bootstrap.soul_seeding_failed");
        }
        if let Err(e) =
            crate::db::seeding::seed_default_user_profiles(repos.user_profile_repo.as_ref()).await
        {
            tracing::warn!(error = %e, "bootstrap.profile_seeding_failed");
        }
    }

    wired.wire_sse_callbacks(&infra.connection_manager).await;

    infra.mdns_announcer.start().await;

    print_banner(&infra.config_arc.load());

    let ollama_install = {
        let data_dir = crate::util::paths::bobe_data_dir();
        let http = Arc::new(
            reqwest::Client::builder()
                .build()
                .map_err(|e| AppError::Internal(format!("reqwest client build: {e}")))?,
        );
        let binary = Arc::new(crate::binary_manager::BinaryManager::new(
            &data_dir,
            Arc::clone(&http),
        ));
        // Strip `/v1` OpenAI-compat suffix to get the native Ollama API root.
        let base_url = config
            .engine
            .provider_base_url
            .as_deref()
            .map(crate::ollama_manager::OllamaManager::root_from_provider_url)
            .unwrap_or_else(|| "http://127.0.0.1:11434".to_string());
        let manager = Arc::new(crate::ollama_manager::OllamaManager::new(
            Arc::clone(&http),
            &base_url,
        ));
        crate::services::ollama_install_service::OllamaInstallService::new(binary, manager)
    };

    let (voice_stt, voice_tts, voice_vad, voice_smart_turn) = load_voice_engines();
    let voice_filler_pcm = match voice_tts.as_ref() {
        Some(tts) => synthesize_filler(Arc::clone(tts)).await,
        None => None,
    };

    let state = Arc::new(AppState {
        db: pool,
        config: Arc::clone(&infra.config_arc),
        event_queue: infra.event_queue,
        connection_manager: infra.connection_manager,
        soul_repo: repos.soul_repo,
        user_profile_repo: repos.user_profile_repo,
        goals_service: wired.goals_service,
        runtime_session: wired.runtime_session,
        screen_capture: wired.screen_capture,
        config_manager: wired.config_manager,
        mcp_config_lock: Arc::new(tokio::sync::Mutex::new(())),
        mdns_announcer: infra.mdns_announcer,
        workers,
        memory_file,
        ollama_install,
        voice_stt,
        voice_tts,
        voice_vad,
        voice_smart_turn,
        voice_filler_pcm,
        voice_turn_active,
    });

    Ok(state)
}

/// Best-effort load of local STT + TTS engines from `~/.bobe/models/`.
/// Returns `(None, None)` if model files are missing — the daemon still
/// starts; `/voice/stream` returns 503 until models are installed.
///
/// Provider selection per platform: macOS → CoreML EP, others → CPU.
/// Env overrides: `BOBE_VOICE_PROVIDER` (cpu|coreml|cuda|directml),
/// `BOBE_VOICE_NUM_THREADS` (default 4).
fn load_voice_engines() -> (
    Option<Arc<dyn crate::speech::SttEngine>>,
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
        let dir = models_root.join("sherpa-onnx-moonshine-base-en-int8");
        if dir.exists() {
            match crate::speech::local_sherpa::LocalSherpaStt::load(&dir, &provider, num_threads) {
                Ok(e) => {
                    info!(path = %dir.display(), provider, "voice.stt_loaded");
                    Some(Arc::new(e) as Arc<dyn crate::speech::SttEngine>)
                }
                Err(e) => {
                    warn!(error = %e, "voice.stt_load_failed");
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

    // Smart-turn stub: real ONNX impl lands with the VAD pipeline (0.c).
    // For now we always supply it so callers can rely on the field being
    // populated; the stub returns 1.0 (always "turn complete") so behavior
    // falls back to pure acoustic silence detection.
    let smart_turn: Option<Arc<dyn crate::speech::SemanticTurn>> = Some(Arc::new(
        crate::speech::local_smart_turn::StubSmartTurn,
    ));

    (stt, tts, vad, smart_turn)
}

/// One-shot filler synthesis at startup. The /voice/stream handler emits this
/// pre-rendered PCM at the 800ms TTFT watchdog trip so users hear an
/// acknowledgement instead of dead air while the LLM is working.
///
/// Single phrase for v1 — full intent-keyed filler library is M4.5.4
/// follow-up. Synthesis blocks startup by ~300-800ms; tolerable, and the
/// alternative (background task + ArcSwap) adds complexity for marginal win.
async fn synthesize_filler(
    tts: Arc<dyn crate::speech::TtsEngine>,
) -> Option<Arc<Vec<f32>>> {
    const PHRASE: &str = "Hmm, let me think.";
    const VOICE: &str = "af_bella";
    let tts_clone = Arc::clone(&tts);
    match tokio::task::spawn_blocking(move || tts_clone.synthesize(PHRASE, VOICE, 1.0)).await {
        Ok(Ok(pcm)) => {
            info!(
                phrase = PHRASE,
                samples = pcm.len(),
                rate = tts.sample_rate(),
                "voice.filler_rendered"
            );
            Some(Arc::new(pcm))
        }
        Ok(Err(e)) => {
            warn!(error = %e, "voice.filler_synth_failed");
            None
        }
        Err(e) => {
            warn!(error = %e, "voice.filler_synth_join_failed");
            None
        }
    }
}

fn load_mcp_servers_for_sdk(
    config: &Config,
) -> std::collections::HashMap<String, github_copilot_sdk::types::McpServerConfig> {
    if !config.mcp.enabled {
        return std::collections::HashMap::new();
    }

    let path =
        match crate::mcp::config::resolve_mcp_config_path(config.mcp.config_file.as_deref())
        {
            Ok(p) => p,
            Err(e) => {
                warn!(error = %e, "bootstrap.mcp_config_path_resolution_failed");
                return std::collections::HashMap::new();
            }
        };

    let parsed = match crate::mcp::config::load_mcp_config(
        &path,
        &config.mcp.blocked_commands,
        &config.mcp.dangerous_env_keys,
    ) {
        Ok(servers) => servers,
        Err(e) => {
            warn!(error = %e, path = %path.display(), "bootstrap.mcp_config_parse_failed");
            return std::collections::HashMap::new();
        }
    };

    let count = parsed.len();
    let map = crate::mcp::config::to_sdk_mcp_servers(parsed);
    info!(count, "bootstrap.mcp_servers_registered_via_sdk");
    map
}

fn print_banner(config: &Config) {
    info!("═══════════════════════════════════════════════════════");
    info!("  BoBe Server Started");
    info!("  Engine: Copilot CLI (via github-copilot-sdk)");
    info!("  Capture enabled: {}", config.capture.enabled);
    info!("  Tools (MCP) enabled: {}", config.mcp.enabled);
    info!("═══════════════════════════════════════════════════════");
}
