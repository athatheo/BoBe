#![allow(dead_code)]

use clap::{Parser, Subcommand};

mod config;
mod error;
mod domain;
mod ports;
mod shared;
mod adapters;
mod application;
mod entrypoints;
mod composition;
mod app_state;

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
        #[arg(short, long, default_value_t = 8765)]
        port: u16,
        /// Log level
        #[arg(short, long, default_value = "INFO")]
        log_level: String,
    },
    /// Interactive setup wizard (requires running server)
    Setup {
        /// Service host
        #[arg(short = 'H', long, default_value = "127.0.0.1")]
        host: String,
        /// Service port
        #[arg(short, long, default_value_t = 8765)]
        port: u16,
    },
    /// Show version information
    Version,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { host, port, log_level } => {
            // SAFETY: set_var is called before any threads are spawned.
            unsafe {
                std::env::set_var("BOBE_HOST", &host);
                std::env::set_var("BOBE_PORT", port.to_string());
                std::env::set_var("BOBE_LOG_LEVEL", &log_level);
            }

            let config = config::Config::from_env()?;

            // Initialize tracing
            adapters::logging::init_tracing(&config);

            tracing::info!("Starting BoBe on {}:{}", config.host, config.port);

            // Bootstrap: create pool, run migrations, seed, wire deps, build state
            let state = composition::bootstrap::run(config.clone()).await?;
            let app = entrypoints::app::build_router(state.clone());

            // ── Background tasks ────────────────────────────────────────
            let (shutdown_tx, _) = tokio::sync::broadcast::channel::<()>(1);

            // SSE heartbeat (every 15s)
            let heartbeat_handle = {
                let eq = state.event_queue.clone();
                let mut shutdown_rx = shutdown_tx.subscribe();
                tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            _ = tokio::time::sleep(std::time::Duration::from_secs(15)) => {
                                eq.push_heartbeat();
                            }
                            _ = shutdown_rx.recv() => break,
                        }
                    }
                    tracing::info!("heartbeat_task.stopped");
                })
            };

            // Runtime session (trigger loop)
            let runtime_handle = {
                let session = state.runtime_session.clone();
                let mut shutdown_rx = shutdown_tx.subscribe();
                tokio::spawn(async move {
                    tokio::select! {
                        _ = session.run() => {}
                        _ = shutdown_rx.recv() => {
                            session.stop().await;
                        }
                    }
                    tracing::info!("runtime_session_task.stopped");
                })
            };

            // Learning loop (if enabled)
            let learning_handle = state.learning_loop.as_ref().map(|ll| {
                let ll = ll.clone();
                let mut shutdown_rx = shutdown_tx.subscribe();
                tokio::spawn(async move {
                    tokio::select! {
                        _ = ll.run() => {}
                        _ = shutdown_rx.recv() => {
                            ll.stop();
                        }
                    }
                    tracing::info!("learning_loop_task.stopped");
                })
            });

            // ── Serve with graceful shutdown ────────────────────────────
            let listener = tokio::net::TcpListener::bind(format!("{}:{}", config.host, config.port)).await?;
            tracing::info!("BoBe listening on {}:{}", config.host, config.port);

            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    tokio::signal::ctrl_c().await.ok();
                    tracing::info!("Shutdown signal received, stopping background tasks...");
                    let _ = shutdown_tx.send(());
                })
                .await?;

            // Wait for background tasks to finish
            let _ = heartbeat_handle.await;
            let _ = runtime_handle.await;
            if let Some(h) = learning_handle {
                let _ = h.await;
            }

            tracing::info!("BoBe shutdown complete");
        }
        Commands::Version => {
            println!("BoBe v{}", env!("CARGO_PKG_VERSION"));
        }
        Commands::Setup { host, port } => {
            run_setup(&host, port).await?;
        }
    }

    Ok(())
}

async fn run_setup(host: &str, port: u16) -> anyhow::Result<()> {
    let base_url = format!("http://{}:{}", host, port);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    println!("\nChecking BoBe setup status...\n");

    let resp = match client.get(format!("{base_url}/api/onboarding/status")).send().await {
        Ok(r) => r,
        Err(_) => {
            eprintln!("Cannot connect to BoBe at {base_url}");
            eprintln!("Start the service first: bobe serve");
            std::process::exit(1);
        }
    };

    let status: serde_json::Value = resp.json().await?;

    if let Some(steps) = status.get("steps").and_then(|s| s.as_object()) {
        for (name, data) in steps {
            let complete = data.get("status").and_then(|s| s.as_str()) == Some("complete");
            let detail = data.get("detail").and_then(|s| s.as_str()).unwrap_or("");
            let icon = if complete { "✓" } else { "✗" };
            println!("  {icon} {name}: {detail}");
        }
    }

    if status.get("complete").and_then(|c| c.as_bool()) == Some(true) {
        println!("\nAll set! BoBe is fully configured.");
        return Ok(());
    }

    // Choose LLM backend
    println!("\nChoose your LLM backend:");
    println!("  1. Ollama (local, private, free)");
    println!("  2. OpenAI (cloud, API key required)");
    println!("  3. Anthropic Claude (cloud, API key required)");

    let mode = prompt("Choice [1]: ").unwrap_or_else(|| "1".into());
    let mode = match mode.trim() {
        "2" => "openai",
        "3" => "anthropic",
        _ => "ollama",
    };

    if mode == "ollama" {
        println!("\nChoose model size:");
        println!("  1. Small  (3B)  — ~2GB, fast, any Mac");
        println!("  2. Medium (14B) — ~8GB, smarter, 32GB+ RAM");
        println!("  3. Large  (32B) — ~20GB, best quality, 64GB+ RAM");

        let choice = prompt("Choice [1]: ").unwrap_or_else(|| "1".into());
        let model = match choice.trim() {
            "2" => "qwen3:14b",
            "3" => "qwen3:32b",
            _ => "llama3.2:3b",
        };

        client
            .post(format!("{base_url}/api/onboarding/configure-llm"))
            .json(&serde_json::json!({"mode": "ollama", "model": model}))
            .send()
            .await?;
        println!("Configured Ollama with {model}");

        println!("\nDownloading {model}...");
        let pull_resp = client
            .post(format!("{base_url}/api/onboarding/pull-model"))
            .json(&serde_json::json!({"model": model}))
            .timeout(std::time::Duration::from_secs(3600))
            .send()
            .await?;

        let body = pull_resp.text().await?;
        for line in body.lines() {
            if let Some(data) = line.strip_prefix("data: ")
                && let Ok(val) = serde_json::from_str::<serde_json::Value>(data) {
                    match val.get("status").and_then(|s| s.as_str()) {
                        Some("complete") => println!("✓ Model downloaded"),
                        Some("error") => {
                            let detail = val.get("detail").and_then(|d| d.as_str()).unwrap_or("unknown");
                            eprintln!("Error: {detail}");
                            std::process::exit(1);
                        }
                        Some("pulling") => {
                            let progress = val.get("progress").and_then(|p| p.as_f64()).unwrap_or(0.0);
                            print!("\r  Downloading: {progress:.0}%    ");
                        }
                        _ => {}
                    }
                }
        }
        println!();
    } else {
        let default_model = if mode == "openai" { "gpt-4o-mini" } else { "claude-sonnet-4-5-20250929" };

        let api_key = prompt("API Key: ").unwrap_or_default();
        let model_input = prompt(&format!("Model [{default_model}]: ")).unwrap_or_default();
        let model = if model_input.trim().is_empty() { default_model } else { model_input.trim() };

        client
            .post(format!("{base_url}/api/onboarding/configure-llm"))
            .json(&serde_json::json!({"mode": mode, "api_key": api_key, "model": model}))
            .send()
            .await?;
        println!("Configured {mode}");
    }

    // Mark complete
    client.post(format!("{base_url}/api/onboarding/mark-complete")).send().await?;
    println!("\n✓ BoBe is ready!\n");

    Ok(())
}

fn prompt(message: &str) -> Option<String> {
    use std::io::Write;
    print!("{message}");
    std::io::stdout().flush().ok()?;
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok()?;
    let trimmed = input.trim().to_string();
    if trimmed.is_empty() { None } else { Some(trimmed) }
}
