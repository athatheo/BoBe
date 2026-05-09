use clap::{Parser, Subcommand};

mod api;
mod app_state;
mod bootstrap;
mod config;
mod config_manager;
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
mod util;

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
        #[arg(short, long, default_value_t = 8766)]
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

            let (shutdown_tx, _) = tokio::sync::broadcast::channel::<()>(8);
            let handles = spawn_background_tasks(&state, &shutdown_tx);

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

            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    tokio::signal::ctrl_c().await.ok();
                    tracing::info!("Shutdown signal received, stopping background tasks...");
                    let _ = shutdown_tx.send(());
                })
                .await?;

            drain_background_tasks(handles).await;
            run_graceful_shutdown(&state, &config).await;
        }
        #[allow(clippy::print_stdout)]
        Commands::Version => {
            println!("BoBe v{}", env!("CARGO_PKG_VERSION"));
        }
    }

    Ok(())
}

struct BackgroundHandles {
    heartbeat: tokio::task::JoinHandle<()>,
    runtime: tokio::task::JoinHandle<()>,
    consolidation: tokio::task::JoinHandle<()>,
}

fn spawn_background_tasks(
    state: &std::sync::Arc<app_state::AppState>,
    shutdown_tx: &tokio::sync::broadcast::Sender<()>,
) -> BackgroundHandles {
    let heartbeat = {
        let eq = std::sync::Arc::clone(&state.event_queue);
        let mut shutdown_rx = shutdown_tx.subscribe();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    () = tokio::time::sleep(std::time::Duration::from_secs(15)) => {
                        eq.push_heartbeat();
                    }
                    _ = shutdown_rx.recv() => break,
                }
            }
            tracing::info!("heartbeat_task.stopped");
        })
    };

    let runtime = {
        let session = std::sync::Arc::clone(&state.runtime_session);
        let mut shutdown_rx = shutdown_tx.subscribe();
        tokio::spawn(async move {
            tokio::select! {
                () = session.run() => {}
                _ = shutdown_rx.recv() => {
                    session.stop().await;
                }
            }
            tracing::info!("runtime_session_task.stopped");
        })
    };

    let consolidation = {
        let trigger = copilot::consolidation::ConsolidationTrigger::new(
            std::sync::Arc::clone(&state.workers),
            std::sync::Arc::clone(&state.memory_file),
        );
        let shutdown_rx = shutdown_tx.subscribe();
        tokio::spawn(async move {
            trigger.run(shutdown_rx).await;
            tracing::info!("consolidation_trigger_task.stopped");
        })
    };

    BackgroundHandles {
        heartbeat,
        runtime,
        consolidation,
    }
}

async fn drain_background_tasks(handles: BackgroundHandles) {
    if let Err(e) = handles.heartbeat.await {
        tracing::error!(error = %e, "heartbeat task panicked");
    }
    if let Err(e) = handles.runtime.await {
        tracing::error!(error = %e, "runtime session task panicked");
    }
    if let Err(e) = handles.consolidation.await {
        tracing::error!(error = %e, "consolidation trigger task panicked");
    }
}

async fn run_graceful_shutdown(
    state: &std::sync::Arc<app_state::AppState>,
    _config: &config::Config,
) {
    tracing::info!("Stopping mDNS...");
    state.mdns_announcer.stop().await;

    tracing::info!("Stopping Copilot workers...");
    state.workers.shutdown_all().await;

    tracing::info!("Closing database pool...");
    state.db.close().await;

    tracing::info!("BoBe shutdown complete");
}
