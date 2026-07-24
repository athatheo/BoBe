use clap::{Parser, Subcommand};
use secrecy::ExposeSecret;
use std::sync::Arc;

mod api;
mod app_state;
mod body;
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
        #[arg(short = 'H', long)]
        host: Option<String>,
        #[arg(short, long)]
        port: Option<u16>,
        #[arg(short, long)]
        log_level: Option<String>,
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
            if let Some(host) = host {
                config.server.host = host;
            }
            if let Some(port) = port {
                config.server.port = port;
            }
            if let Some(log_level) = log_level {
                config.logging.level = log_level;
            }

            util::logging::init_tracing(&config);

            tracing::info!(
                "Starting BoBe on {}:{}",
                config.server.host,
                config.server.port
            );

            validate_server_security(&config)?;
            let shutdown = tokio_util::sync::CancellationToken::new();
            let state = bootstrap::run(config.clone()).await?;
            api::handlers::conversation::recover_message_requests(&state).await?;
            let app = api::router::build_router(std::sync::Arc::clone(&state), shutdown.clone());

            let mut tasks = spawn_background_tasks(&state, &shutdown);

            let address = server_socket_addr(&config)?;
            tracing::info!(%address, "BoBe listening");

            let handle = axum_server::Handle::new();
            let shutdown_handle = handle.clone();
            let shutdown_for_server = shutdown.clone();
            tokio::spawn(async move {
                wait_for_shutdown_signal().await;
                tracing::info!("Shutdown signal received, stopping background tasks...");
                shutdown_for_server.cancel();
                shutdown_handle.graceful_shutdown(Some(std::time::Duration::from_secs(5)));
            });

            if let (Some(cert), Some(key)) = (
                config.server.tls_cert_path.as_deref(),
                config.server.tls_key_path.as_deref(),
            ) {
                let tls = axum_server::tls_rustls::RustlsConfig::from_pem_file(cert, key).await?;
                axum_server::bind_rustls(address, tls)
                    .handle(handle)
                    .serve(app.into_make_service())
                    .await?;
            } else {
                axum_server::bind(address)
                    .handle(handle)
                    .serve(app.into_make_service())
                    .await?;
            }

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

fn server_socket_addr(config: &config::Config) -> anyhow::Result<std::net::SocketAddr> {
    let ip = config.server.host.parse::<std::net::IpAddr>()?;
    Ok(std::net::SocketAddr::new(ip, config.server.port))
}

fn validate_server_security(config: &config::Config) -> anyhow::Result<()> {
    let host = config.server.host.parse::<std::net::IpAddr>()?;
    let exposes_public_host = !config.server.allowed_hosts.is_empty();
    if host.is_loopback() && !exposes_public_host {
        return Ok(());
    }
    anyhow::ensure!(
        !config.server.allowed_hosts.is_empty(),
        "non-loopback bind requires BOBE_SERVER__ALLOWED_HOSTS"
    );
    anyhow::ensure!(
        !config.server.api_token.expose_secret().is_empty(),
        "remote or reverse-proxy access requires BOBE_SERVER__API_TOKEN"
    );
    anyhow::ensure!(
        config.server.tls_cert_path.is_some() && config.server.tls_key_path.is_some(),
        "remote or reverse-proxy access requires BOBE_SERVER__TLS_CERT_PATH and BOBE_SERVER__TLS_KEY_PATH"
    );
    Ok(())
}

async fn wait_for_shutdown_signal() {
    #[cfg(unix)]
    {
        let mut terminate =
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                Ok(signal) => signal,
                Err(error) => {
                    tracing::warn!(%error, "failed to install SIGTERM handler; waiting for Ctrl-C");
                    if let Err(error) = tokio::signal::ctrl_c().await {
                        tracing::error!(%error, "failed to wait for Ctrl-C");
                    }
                    return;
                }
            };
        tokio::select! {
            result = tokio::signal::ctrl_c() => {
                if let Err(error) = result {
                    tracing::error!(%error, "failed to wait for Ctrl-C");
                }
            }
            _ = terminate.recv() => {}
        }
    }

    #[cfg(not(unix))]
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(%error, "failed to wait for Ctrl-C");
    }
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
            std::sync::Arc::clone(&state.runtime.runtime_session),
        );
        let token = shutdown.clone();
        set.spawn(async move {
            scheduler.run(token).await;
            "consolidation"
        });
    }

    {
        let pool = state.infra.db.clone();
        let config = std::sync::Arc::clone(&state.infra.config);
        let token = shutdown.clone();
        set.spawn(async move {
            loop {
                crate::services::storage_retention::run(&pool, &config.load()).await;
                tokio::select! {
                    () = tokio::time::sleep(crate::services::storage_retention::RETENTION_INTERVAL) => {}
                    () = token.cancelled() => break,
                }
            }
            "storage_retention"
        });
    }

    {
        let body_config = state.config().body.clone();
        if body_config.enabled {
            let gateway = Arc::clone(&state.body.gateway);
            let status_gateway = Arc::clone(&gateway);
            let token = shutdown.clone();
            set.spawn(async move {
                if let Err(error) = body::server::run(gateway, body_config, token).await {
                    status_gateway.mark_failed();
                    tracing::error!(%error, "body.gateway_failed");
                }
                "body_gateway"
            });
        }
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
                tracing::warn!(remaining, "background task drain timeout, abandoning");
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

#[cfg(test)]
#[allow(clippy::expect_used, reason = "tests panic on invalid CLI fixtures")]
mod tests {
    use super::*;

    #[test]
    fn non_loopback_requires_complete_security_configuration() {
        let mut config = config::Config::default();
        config.server.host = "0.0.0.0".into();
        assert!(validate_server_security(&config).is_err());

        config.server.allowed_hosts = vec!["bobe.example.test".into()];
        config.server.api_token = secrecy::SecretString::from("secret".to_string());
        config.server.tls_cert_path = Some("cert.pem".into());
        config.server.tls_key_path = Some("key.pem".into());
        assert!(validate_server_security(&config).is_ok());
    }

    #[test]
    fn loopback_does_not_require_remote_transport_credentials() {
        let config = config::Config::default();
        assert!(validate_server_security(&config).is_ok());
    }

    #[test]
    fn loopback_with_public_allowed_host_requires_credentials() {
        let mut config = config::Config::default();
        config.server.allowed_hosts = vec!["bobe.example.test".into()];
        assert!(validate_server_security(&config).is_err());
    }

    #[test]
    fn socket_address_supports_ipv4_and_ipv6_structurally() {
        let mut config = config::Config::default();
        assert_eq!(
            server_socket_addr(&config).expect("valid IPv4"),
            "127.0.0.1:8766".parse().expect("fixture")
        );
        config.server.host = "::1".into();
        assert_eq!(
            server_socket_addr(&config).expect("valid IPv6"),
            "[::1]:8766".parse().expect("fixture")
        );
    }

    #[test]
    fn serve_cli_leaves_config_overrides_unset_by_default() {
        let cli = Cli::try_parse_from(["bobe", "serve"]).expect("valid serve command");
        match cli.command {
            Commands::Serve {
                host,
                port,
                log_level,
            } => {
                assert!(host.is_none());
                assert!(port.is_none());
                assert!(log_level.is_none());
            }
            Commands::Version => panic!("expected serve command"),
        }
    }

    #[test]
    fn serve_cli_accepts_explicit_overrides() {
        let cli = Cli::try_parse_from([
            "bobe",
            "serve",
            "--host",
            "127.0.0.2",
            "--port",
            "9000",
            "--log-level",
            "debug",
        ])
        .expect("valid serve overrides");
        match cli.command {
            Commands::Serve {
                host,
                port,
                log_level,
            } => {
                assert_eq!(host.as_deref(), Some("127.0.0.2"));
                assert_eq!(port, Some(9000));
                assert_eq!(log_level.as_deref(), Some("debug"));
            }
            Commands::Version => panic!("expected serve command"),
        }
    }
}
