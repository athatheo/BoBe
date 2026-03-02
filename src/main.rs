use clap::{Parser, Subcommand};

mod api;
mod app_state;
mod binary_manager;
mod bootstrap;
mod config;
mod config_manager;
mod db;
mod error;
mod llm;
mod models;
mod runtime;
#[allow(unsafe_code)]
mod secrets;
mod services;
mod tools;
mod util;

#[derive(Parser)]
#[command(name = "bobe", about = "BoBe - Local-first proactive AI companion")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the BoBe server
    Serve {
        /// Host to bind to
        #[arg(short = 'H', long, default_value = "127.0.0.1")]
        host: String,
        /// Port to bind to
        #[arg(short, long, default_value_t = 8766)]
        port: u16,
        /// Log level
        #[arg(short, long, default_value = "INFO")]
        log_level: String,
    },
    /// Show version information
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

            // Initialize tracing
            util::logging::init_tracing(&config);

            tracing::info!(
                "Starting BoBe on {}:{}",
                config.server.host,
                config.server.port
            );

            // Bootstrap: create pool, run migrations, seed, wire deps, build state
            let (state, goal_worker_manager) = bootstrap::run(config.clone()).await?;
            let app = api::router::build_router(state.clone());

            // ── Background tasks ────────────────────────────────────────
            let (shutdown_tx, _) = tokio::sync::broadcast::channel::<()>(8);
            let handles = spawn_background_tasks(&state, goal_worker_manager, &shutdown_tx);

            // ── Serve with graceful shutdown ────────────────────────────
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
        Commands::Version => {
            println!("BoBe v{}", env!("CARGO_PKG_VERSION"));
        }
    }

    Ok(())
}

struct BackgroundHandles {
    heartbeat: tokio::task::JoinHandle<()>,
    runtime: tokio::task::JoinHandle<()>,
    learning: Option<tokio::task::JoinHandle<()>>,
    goal_worker: tokio::task::JoinHandle<()>,
}

fn spawn_background_tasks(
    state: &std::sync::Arc<app_state::AppState>,
    goal_worker_manager: services::goal_worker::manager::GoalWorkerManager,
    shutdown_tx: &tokio::sync::broadcast::Sender<()>,
) -> BackgroundHandles {
    let heartbeat = {
        let eq = state.event_queue.clone();
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
        let session = state.runtime_session.clone();
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

    let learning = state.learning_loop.as_ref().map(|ll| {
        let ll = ll.clone();
        let mut shutdown_rx = shutdown_tx.subscribe();
        tokio::spawn(async move {
            tokio::select! {
                () = ll.run() => {}
                _ = shutdown_rx.recv() => {
                    ll.stop();
                }
            }
            tracing::info!("learning_loop_task.stopped");
        })
    });

    let goal_worker = {
        let shutdown_rx = shutdown_tx.subscribe();
        let mut manager = goal_worker_manager;
        tokio::spawn(async move {
            manager.run(shutdown_rx).await;
            tracing::info!("goal_worker_manager_task.stopped");
        })
    };

    BackgroundHandles {
        heartbeat,
        runtime,
        learning,
        goal_worker,
    }
}

async fn drain_background_tasks(handles: BackgroundHandles) {
    if let Err(e) = handles.heartbeat.await {
        tracing::error!(error = %e, "heartbeat task panicked");
    }
    if let Err(e) = handles.runtime.await {
        tracing::error!(error = %e, "runtime session task panicked");
    }
    if let Some(h) = handles.learning
        && let Err(e) = h.await
    {
        tracing::error!(error = %e, "learning loop task panicked");
    }
    if let Err(e) = handles.goal_worker.await {
        tracing::error!(error = %e, "goal worker manager task panicked");
    }
}

async fn run_graceful_shutdown(
    state: &std::sync::Arc<app_state::AppState>,
    config: &config::Config,
) {
    tracing::info!("Stopping mDNS...");
    state.mdns_announcer.stop().await;

    if let Some(ref mcp) = state.mcp_tool_adapter {
        tracing::info!("Stopping MCP servers...");
        tokio::time::timeout(std::time::Duration::from_secs(2), mcp.shutdown())
            .await
            .ok();
    }

    if config.llm.backend == crate::config::LlmBackend::Ollama
        || config.vision.backend == crate::config::LlmBackend::Ollama
    {
        tracing::info!("Unloading Ollama models...");
        let unload_client = reqwest::Client::new();
        for model_name in [
            &config.ollama.model,
            &config.vision.ollama_model,
            &config.embedding.model,
        ] {
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(2),
                unload_client
                    .post(format!("{}/api/generate", config.ollama.url))
                    .json(&serde_json::json!({"model": model_name, "keep_alive": 0}))
                    .send(),
            )
            .await;
        }
        tracing::debug!("ollama.models_unloaded");

        tracing::info!("Stopping Ollama (if managed)...");
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            state.ollama_manager.stop(),
        )
        .await
        .ok();
    }

    tracing::info!("Closing database pool...");
    state.db.close().await;

    tracing::info!("BoBe shutdown complete");
}
