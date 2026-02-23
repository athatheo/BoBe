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
            let app = entrypoints::app::build_router(state);

            let listener = tokio::net::TcpListener::bind(format!("{}:{}", config.host, config.port)).await?;
            tracing::info!("BoBe listening on {}:{}", config.host, config.port);
            axum::serve(listener, app).await?;
        }
        Commands::Version => {
            println!("BoBe v{}", env!("CARGO_PKG_VERSION"));
        }
    }

    Ok(())
}
