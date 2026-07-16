mod database;
mod infra;
mod legacy_migration;
pub(crate) mod mcp_loader;
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
    crate::util::tls::install_crypto_provider()?;
    let pool = database::connect_and_apply_schema(&config.database.url).await?;
    let data_root = std::path::PathBuf::from(&config.data_dir);
    legacy_migration::run(&pool, &data_root).await?;

    let infra = infra::Infrastructure::build(&config)?;
    let repos = repos::Repositories::from_pool(&pool);

    if config.mcp.enabled
        && let Err(e) = crate::mcp::config::ensure_mcp_config_exists(
            &data_root,
            config.mcp.config_file.as_deref(),
        )
    {
        warn!(error = %e, "bootstrap.ensure_mcp_config_failed");
    }

    let memory_file = {
        let path = data_root.join("memory.md");
        crate::copilot::memory_file::MemoryFile::new(path)
    };
    // Existing skill files are never overwritten.
    crate::copilot::skills::ensure_skills(&data_root).await;
    let mcp = load_mcp_servers_for_sdk(&config);
    // Shared by AppState (consumed by voice.rs) and WorkerRegistry (BobeHooks).
    let voice_turn_active = Arc::new(std::sync::atomic::AtomicBool::new(false));
    // Engines aren't concurrent-feed safe; CAS in voice.rs admits one at a time.
    let voice_ws_active = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let voice_sink = Arc::new(crate::voice::sinks::VoiceSink::new());
    // Prometheus recorder is process-global; install once.
    let metrics_handle = crate::voice::telemetry::install_recorder()
        .map_err(|e| AppError::Internal(format!("metrics recorder: {e}")))?;
    // ArcSwap so the installer can hot-swap post-download without daemon restart.
    // Start empty and spawn the engine load — TTS ONNX init + filler synthesis
    // adds 1-3s to bootstrap on CPU. Voice readiness is gated on `is_complete()`
    // already, so a connection arriving before the swap sees "not ready" and
    // the client retries. Listener bind doesn't have to wait on it.
    let voice_engines: Arc<ArcSwap<crate::voice::engines::VoiceEnginesSnapshot>> = Arc::new(
        ArcSwap::from_pointee(crate::voice::engines::VoiceEnginesSnapshot::default()),
    );
    {
        let voice_engines = Arc::clone(&voice_engines);
        tokio::spawn(async move {
            let snap = build_voice_engines_snapshot().await;
            info!(
                complete = snap.is_complete(),
                "bootstrap.voice_engines_loaded"
            );
            voice_engines.store(Arc::new(snap));
        });
    }
    let workers = {
        let data_dir = data_root.clone();
        crate::copilot::registry::WorkerRegistry::new(
            Arc::clone(&infra.config_arc),
            Arc::clone(&memory_file),
            data_dir,
            mcp.servers,
            mcp.excluded_tools,
            Arc::clone(&voice_turn_active),
            Arc::clone(&voice_sink),
            Arc::clone(&voice_engines),
        )
    };

    let wired = wiring::wire(&config, &infra, &repos, Arc::clone(&workers)).await;

    // Hard reload only when SDK env changes (engine/provider/offline); model
    // changes go soft to preserve chat context. Defers up to 30s mid-voice
    // turn — otherwise PATCH /settings stops the LLM stream and the user
    // hears half a sentence.
    {
        let registry_for_listener = Arc::clone(&workers);
        let voice_turn_active_for_listener = Arc::clone(&voice_turn_active);
        wired
            .config_manager
            .set_engine_change_listener(move |kind| {
                let registry = Arc::clone(&registry_for_listener);
                let voice_active = Arc::clone(&voice_turn_active_for_listener);
                tokio::spawn(async move {
                    wait_for_voice_idle(&voice_active).await;
                    match kind {
                        crate::config::manager::EngineChangeKind::Hard => registry.reload().await,
                        crate::config::manager::EngineChangeKind::Soft => {
                            registry.reload_soft().await;
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

    // Shared outbound client. `read_timeout` resets per-read so stalled
    // downloads die at the TLS layer; no whole-response timeout because
    // voice install streams ~340 MB on slow connections.
    let http_client = Arc::new(
        reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .read_timeout(std::time::Duration::from_secs(30))
            .user_agent(concat!(
                "bobe-daemon/",
                env!("CARGO_PKG_VERSION"),
                " (+https://www.bobebot.com)"
            ))
            .build()?,
    );

    let ollama_install = {
        let data_dir = data_root.clone();
        let binary = Arc::new(crate::services::ollama::binary_manager::BinaryManager::new(
            &data_dir,
            Arc::clone(&http_client),
        ));
        // Strip `/v1` OpenAI-compat suffix to get the native Ollama API root.
        let base_url = config.engine.provider_base_url.as_deref().map_or_else(
            || crate::constants::DEFAULT_OLLAMA_BASE_URL.to_string(),
            crate::services::ollama::manager::OllamaManager::root_from_provider_url,
        );
        let manager = Arc::new(crate::services::ollama::manager::OllamaManager::new(
            Arc::clone(&http_client),
            &base_url,
        ));
        crate::services::ollama::install_service::OllamaInstallService::new(binary, manager)
    };

    let voice_install = {
        let http = Arc::clone(&http_client);
        let models_root = data_root.join("models");
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

    let secret_store = crate::secrets::default_secret_store();

    let in_flight_text_turns = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let copilot_login = {
        let workers = Arc::clone(&workers);
        crate::copilot::login::LoginCoordinator::new(Arc::new(move || {
            let workers = Arc::clone(&workers);
            tokio::spawn(async move { workers.reload_soft().await });
        }))
    };

    let state = Arc::new(AppState {
        infra: Arc::new(crate::app_state::Infrastructure {
            db: pool,
            config: Arc::clone(&infra.config_arc),
            config_manager: wired.config_manager,
            event_queue: infra.event_queue,
            connection_manager: infra.connection_manager,
            mdns_announcer: infra.mdns_announcer,
            secret_store,
            metrics_handle,
            http_client,
            mcp_config_lock: Arc::new(tokio::sync::Mutex::new(())),
        }),
        runtime: Arc::new(crate::app_state::RuntimeContext {
            runtime_session: wired.runtime_session,
            workers,
            memory_file,
            in_flight_text_turns,
        }),
        voice: Arc::new(crate::app_state::VoiceContext {
            voice_install,
            voice_engines,
            voice_turn_active,
            voice_ws_active,
            voice_sink,
        }),
        services: Arc::new(crate::app_state::DomainServices {
            souls_service: wired.souls_service,
            user_profile_service: wired.user_profile_service,
            goals_service: wired.goals_service,
            ollama_install,
        }),
        auth: Arc::new(crate::app_state::AuthContext { copilot_login }),
    });

    // Cold-start prewarm: the first chat message was paying for `Client::start`
    // (copilot-cli subprocess spawn) + session creation + MCP connection on the
    // user's send path. Fire-and-forget it now so the SDK is hot by the time
    // the user types their first prompt. Failures are non-fatal — the lazy
    // path still works on the user's first send, just slower.
    {
        let workers = Arc::clone(&state.runtime.workers);
        tokio::spawn(async move {
            match workers.chat().await {
                Ok(_) => tracing::info!("bootstrap.chat_prewarm.ok"),
                Err(e) => tracing::warn!(err = %e, "bootstrap.chat_prewarm.failed"),
            }
        });
    }

    Ok(state)
}

/// Poll-wait until the voice turn flag clears, capped so a wedged turn
/// doesn't pin engine-config reloads forever. 30s is well past the longest
/// expected LLM-stream-to-TTS-end span (Copilot SDK chat hard-times out
/// earlier than that anyway), so reaching the deadline means the in-flight
/// turn is misbehaving — log it and reload anyway.
async fn wait_for_voice_idle(voice_turn_active: &std::sync::atomic::AtomicBool) {
    use std::sync::atomic::Ordering;
    const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);
    const DEADLINE: std::time::Duration = std::time::Duration::from_secs(30);
    let start = tokio::time::Instant::now();
    while voice_turn_active.load(Ordering::Acquire) {
        if start.elapsed() >= DEADLINE {
            warn!("config.engine_reload_voice_idle_timeout");
            return;
        }
        tokio::time::sleep(POLL_INTERVAL).await;
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
