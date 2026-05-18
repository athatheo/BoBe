use clap::{Parser, Subcommand};

mod api;
mod app_state;
mod bootstrap;
mod config;
mod constants;
mod copilot;
mod db;
mod error;
mod mcp;
mod models;
mod runtime;
#[allow(unsafe_code)]
mod secrets;
mod services;
mod speech;
mod util;
mod voice;

#[derive(Parser)]
#[command(name = "bobe", about = "BoBe - Local-first proactive AI companion")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Serve {
        #[arg(short = 'H', long, default_value = "127.0.0.1")]
        host: String,
        #[arg(short, long, default_value_t = constants::DEFAULT_DAEMON_PORT)]
        port: u16,
        #[arg(short, long, default_value = "INFO")]
        log_level: String,
    },
    Version,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Serve {
            host,
            port,
            log_level,
        } => {
            let mut config = config::Config::load()?;
            config.server.host = host;
            config.server.port = port;
            config.logging.level = log_level;

            util::logging::init_tracing(&config);

            tracing::info!(
                "Starting BoBe on {}:{}",
                config.server.host,
                config.server.port
            );

            let state = bootstrap::run(config.clone()).await?;
            let app = api::router::build_router(std::sync::Arc::clone(&state));

            // One token; spawns get children so subsets can shut independently later.
            let shutdown = tokio_util::sync::CancellationToken::new();
            let mut tasks = spawn_background_tasks(&state, &shutdown);

            let listener = tokio::net::TcpListener::bind(format!(
                "{}:{}",
                config.server.host, config.server.port
            ))
            .await?;
            tracing::info!(
                "BoBe listening on {}:{}",
                config.server.host,
                config.server.port
            );

            let shutdown_for_axum = shutdown.clone();
            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    tokio::signal::ctrl_c().await.ok();
                    tracing::info!("Shutdown signal received, stopping background tasks...");
                    shutdown_for_axum.cancel();
                })
                .await?;

            drain_background_tasks(&mut tasks).await;
            run_graceful_shutdown(&state, &config).await;
        }
        #[allow(clippy::print_stdout)]
        Commands::Version => {
            println!("BoBe v{}", env!("CARGO_PKG_VERSION"));
        }
    }

    Ok(())
}

/// Heartbeat (15s) + RuntimeSession + nightly consolidation.
fn spawn_background_tasks(
    state: &std::sync::Arc<app_state::AppState>,
    shutdown: &tokio_util::sync::CancellationToken,
) -> tokio::task::JoinSet<&'static str> {
    let mut set = tokio::task::JoinSet::new();

    {
        let eq = std::sync::Arc::clone(&state.infra.event_queue);
        let token = shutdown.clone();
        set.spawn(async move {
            loop {
                tokio::select! {
                    () = tokio::time::sleep(std::time::Duration::from_secs(15)) => {
                        eq.push_heartbeat();
                    }
                    () = token.cancelled() => break,
                }
            }
            "heartbeat"
        });
    }

    {
        let session = std::sync::Arc::clone(&state.runtime.runtime_session);
        let token = shutdown.clone();
        set.spawn(async move {
            tokio::select! {
                () = session.run() => {}
                () = token.cancelled() => {
                    session.stop().await;
                }
            }
            "runtime_session"
        });
    }

    {
        let scheduler = runtime::consolidation::ConsolidationScheduler::new(
            std::sync::Arc::clone(&state.runtime.workers),
            std::sync::Arc::clone(&state.runtime.memory_file),
        );
        let token = shutdown.clone();
        set.spawn(async move {
            scheduler.run(token).await;
            "consolidation"
        });
    }

    set
}

async fn drain_background_tasks(tasks: &mut tokio::task::JoinSet<&'static str>) {
    // Backstop for misbehaving tasks; well-behaved ones honor the cancel token.
    const DRAIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
    let deadline = tokio::time::Instant::now() + DRAIN_TIMEOUT;

    while !tasks.is_empty() {
        match tokio::time::timeout_at(deadline, tasks.join_next()).await {
            Ok(Some(Ok(name))) => tracing::info!(task = name, "background_task.stopped"),
            Ok(Some(Err(e))) => tracing::error!(error = %e, "background task panicked"),
            Ok(None) => break,
            Err(_) => {
                let remaining = tasks.len();
                tracing::warn!(
                    remaining,
                    "background task drain timeout, abandoning"
                );
                tasks.abort_all();
                return;
            }
        }
    }
}

async fn drain_in_flight_text_turns(counter: &std::sync::atomic::AtomicUsize) {
    use std::sync::atomic::Ordering;
    const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(50);
    const DEADLINE: std::time::Duration = std::time::Duration::from_secs(10);
    let start = tokio::time::Instant::now();
    loop {
        let n = counter.load(Ordering::Acquire);
        if n == 0 {
            return;
        }
        if start.elapsed() >= DEADLINE {
            tracing::warn!(in_flight = n, "text_turn_drain_timeout");
            return;
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

async fn run_graceful_shutdown(
    state: &std::sync::Arc<app_state::AppState>,
    _config: &config::Config,
) {
    tracing::info!("Stopping mDNS...");
    state.infra.mdns_announcer.stop().await;

    // Fire the SSE on_disconnect callback so RuntimeSession::on_disconnection
    // runs (stops capture). Writers polling `is_active_connection` see
    // connected=false on their next iteration and exit. Passing None skips
    // the stale-id guard so this works regardless of which connection is
    // current.
    tracing::info!("Signaling SSE clients to disconnect...");
    state.infra.connection_manager.disconnect(None).await;

    // Cancel + drain any in-flight install jobs so they don't outlive
    // the resources they depend on (db, http client, file system handles).
    tracing::info!("Cancelling in-flight installs...");
    state.voice.voice_install.cancel().await;
    state.voice.voice_install.await_idle().await;

    tracing::info!("Stopping Copilot workers...");
    state.runtime.workers.shutdown_all().await;

    // Wait for in-flight text-turn tasks to finish. The SDK abort above
    // makes their LLM streams fail fast, so they should drop within ms.
    // 10s deadline backstops a misbehaving stream.
    drain_in_flight_text_turns(&state.runtime.in_flight_text_turns).await;

    tracing::info!("Closing database pool...");
    state.infra.db.close().await;

    tracing::info!("BoBe shutdown complete");
}
