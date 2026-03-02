use std::path::PathBuf;
use std::sync::OnceLock;

use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::Config;

static LOG_GUARD: OnceLock<WorkerGuard> = OnceLock::new();

pub fn init_tracing(config: &Config) {
    let filter =
        EnvFilter::try_new(&config.logging.level).unwrap_or_else(|_| EnvFilter::new("info"));
    let file_writer = build_file_writer(config.logging.file.as_deref());

    if config.logging.json {
        if let Some(writer) = file_writer {
            tracing_subscriber::registry()
                .with(filter)
                .with(fmt::layer().json().with_writer(writer))
                .init();
        } else {
            tracing_subscriber::registry()
                .with(filter)
                .with(fmt::layer().json())
                .init();
        }
    } else if let Some(writer) = file_writer {
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().with_target(true).with_writer(writer))
            .init();
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().with_target(true))
            .init();
    }
}

fn build_file_writer(log_file: Option<&str>) -> Option<NonBlocking> {
    let path = log_file?.trim();
    if path.is_empty() {
        return None;
    }

    let file_path = PathBuf::from(path);
    let Some(file_name_os) = file_path.file_name() else {
        eprintln!("Invalid log_file path '{path}'; using stdout logging.");
        return None;
    };
    let file_name = file_name_os.to_string_lossy().to_string();
    if file_name.is_empty() {
        eprintln!("Invalid log_file path '{path}'; using stdout logging.");
        return None;
    }

    let dir = file_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), std::path::Path::to_path_buf);

    if let Err(err) = std::fs::create_dir_all(&dir) {
        eprintln!(
            "Failed to create log directory '{}': {err}; using stdout logging.",
            dir.display()
        );
        return None;
    }

    let appender = tracing_appender::rolling::never(dir, file_name);
    let (writer, guard) = tracing_appender::non_blocking(appender);
    if LOG_GUARD.set(guard).is_err() {
        eprintln!("Tracing worker guard was already initialized.");
    }
    Some(writer)
}
