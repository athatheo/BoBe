mod database;
mod infra;
mod mcp_loader;
mod repos;
mod voice_loader;
mod wiring;

use std::sync::Arc;

use arc_swap::ArcSwap;
use tracing::{info, warn};

use crate::app_state::AppState;
use crate::config::Config;
use crate::error::AppError;

use mcp_loader::load_mcp_servers_for_sdk;
use voice_loader::build_voice_engines_snapshot;

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
    // One-shot cleanup of files left behind after the Mode A rip-out
    // (Zipformer/Silero/SmartTurn no longer used; daemon = TTS only).
    // Idempotent — silently no-ops once the files are gone.
    cleanup_legacy_mode_a_files();
    // Voice-turn signal lives here so AppState (consumed by voice.rs) and
    // WorkerRegistry (consumed by BobeHooks) both reference the same Arc.
    let voice_turn_active = Arc::new(std::sync::atomic::AtomicBool::new(false));
    // Single-flight permit for /voice/stream. Engines are not safe for
    // concurrent feed; CAS true→false in voice.rs::handle_socket admits.
    let voice_ws_active = Arc::new(std::sync::atomic::AtomicBool::new(false));
    // Single-slot voice sink — same dual-consumer pattern as the flag.
    let voice_sink = Arc::new(crate::voice::sinks::VoiceSink::new());
    // Install the Prometheus recorder once at boot. The handle goes on
    // AppState; the `/metrics` route renders from it on each request.
    let metrics_handle = crate::voice::telemetry::install_recorder()
        .map_err(|e| AppError::Internal(format!("metrics recorder: {e}")))?;
    // Voice engines under one ArcSwap so the install service can hot-swap
    // post-download. Workers + hooks reference the snapshot Arc; reads at
    // hook-fire time pick up the latest filler library without restart.
    let initial_voice_engines = build_voice_engines_snapshot().await;
    let voice_engines: Arc<ArcSwap<crate::voice::engines::VoiceEnginesSnapshot>> =
        Arc::new(ArcSwap::from_pointee(initial_voice_engines));
    let workers = {
        let data_dir = crate::util::paths::bobe_data_dir();
        crate::copilot::registry::WorkerRegistry::new(
            Arc::clone(&infra.config_arc),
            Arc::clone(&memory_file),
            data_dir,
            mcp_servers,
            Arc::clone(&voice_turn_active),
            Arc::clone(&voice_sink),
            Arc::clone(&voice_engines),
        )
    };

    let wired = wiring::wire(&config, &infra, &repos, Arc::clone(&workers)).await;

    // Engine config changes: hard reload only when the SDK process itself needs new env
    // (engine type, provider URL, offline). Model/reasoning changes use a soft reload that
    // preserves the chat session so the user doesn't lose context.
    {
        let registry_for_listener = Arc::clone(&workers);
        wired
            .config_manager
            .set_engine_change_listener(move |kind| {
                let registry = Arc::clone(&registry_for_listener);
                tokio::spawn(async move {
                    match kind {
                        crate::config_manager::EngineChangeKind::Hard => registry.reload().await,
                        crate::config_manager::EngineChangeKind::Soft => {
                            registry.reload_soft().await
                        }
                    }
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
        if let Err(e) =
            crate::services::goals::seeding::seed_sample_goal(wired.goals_service.as_ref()).await
        {
            tracing::warn!(error = %e, "bootstrap.goal_seeding_failed");
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
        let base_url = config.engine.provider_base_url.as_deref().map_or_else(
            || crate::constants::DEFAULT_OLLAMA_BASE_URL.to_string(),
            crate::ollama_manager::OllamaManager::root_from_provider_url,
        );
        let manager = Arc::new(crate::ollama_manager::OllamaManager::new(
            Arc::clone(&http),
            &base_url,
        ));
        crate::services::ollama_install_service::OllamaInstallService::new(binary, manager)
    };

    let voice_install = {
        let http = reqwest::Client::builder()
            .build()
            .map_err(|e| AppError::Internal(format!("voice install http client: {e}")))?;
        let models_root = dirs::home_dir()
            .map(|h| h.join(".bobe").join("models"))
            .ok_or_else(|| AppError::Internal("no home_dir for voice models root".into()))?;
        let engines_for_reload = Arc::clone(&voice_engines);
        let on_complete: Arc<dyn Fn() -> futures::future::BoxFuture<'static, ()> + Send + Sync> =
            Arc::new(move || {
                let engines = Arc::clone(&engines_for_reload);
                Box::pin(async move {
                    let snap = build_voice_engines_snapshot().await;
                    info!(
                        complete = snap.is_complete(),
                        "voice_install.engines_reloaded"
                    );
                    engines.store(Arc::new(snap));
                })
            });
        crate::voice::install_service::VoiceInstallService::new(http, models_root, on_complete)
    };

    let secret_store: Arc<dyn crate::secrets::SecretStore> =
        Arc::new(crate::secrets::KeychainSecretStore);

    let state = Arc::new(AppState {
        db: pool,
        config: Arc::clone(&infra.config_arc),
        event_queue: infra.event_queue,
        connection_manager: infra.connection_manager,
        souls_service: wired.souls_service,
        user_profile_service: wired.user_profile_service,
        goals_service: wired.goals_service,
        runtime_session: wired.runtime_session,
        config_manager: wired.config_manager,
        mcp_config_lock: Arc::new(tokio::sync::Mutex::new(())),
        mdns_announcer: infra.mdns_announcer,
        workers,
        memory_file,
        ollama_install,
        secret_store,
        voice_install,
        voice_engines,
        voice_turn_active,
        voice_ws_active,
        voice_sink,
        metrics_handle,
    });

    Ok(state)
}

/// Delete on-disk model files left behind after the Mode A rip-out. The
/// daemon now only needs Kokoro TTS (Mode B): STT/VAD/smart-turn moved to
/// the Swift client (FluidAudio). The legacy Zipformer/Silero/SmartTurn
/// downloads (~90MB combined) just take up space if they survived the
/// model-catalog change. Idempotent — silent no-op once cleaned up.
fn cleanup_legacy_mode_a_files() {
    let Some(home) = dirs::home_dir() else { return };
    let models = home.join(".bobe").join("models");
    let legacy: &[(&str, bool)] = &[
        // (path under models/, is_dir)
        ("sherpa-onnx-streaming-zipformer-en", true),
        ("silero-vad", true),
        ("smart-turn-v3.2-cpu.onnx", false),
    ];
    for (rel, is_dir) in legacy {
        let path = models.join(rel);
        if !path.exists() {
            continue;
        }
        let result = if *is_dir {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        match result {
            Ok(()) => info!(path = %path.display(), "voice.legacy_mode_a_file_removed"),
            Err(e) => warn!(path = %path.display(), error = %e, "voice.legacy_mode_a_remove_failed"),
        }
    }
}

/// Top-of-boot info banner.
fn print_banner(config: &Config) {
    info!("═══════════════════════════════════════════════════════");
    info!("  BoBe Server Started");
    info!("  Engine: Copilot CLI (via github-copilot-sdk)");
    info!("  Capture enabled: {}", config.capture.enabled);
    info!("  Tools (MCP) enabled: {}", config.mcp.enabled);
    info!("═══════════════════════════════════════════════════════");
}
