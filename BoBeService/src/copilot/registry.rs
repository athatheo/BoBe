//! `WorkerRegistry` — owns long-lived `CopilotWorker` instances keyed by
//! class name. Workers are spawned lazily on first request (saves boot
//! latency for daemons that never need vision, etc.) and reused on
//! subsequent calls. `shutdown_all` is a single graceful stop.

#![allow(
    dead_code,
    reason = "Phase 2: registry + spec types; consumers cut over in Phase 5"
)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use crate::error::AppError;

use super::agent_worker::AgentWorker;
use super::memory_file::MemoryFile;
use super::mux::Mux;
use super::worker::{CopilotWorker, WorkerConfig};

/// Defaults that every worker class shares unless overridden in
/// `WorkerSpec`. Centralized so changes (e.g. a new Copilot CLI flag)
/// land in one place.
pub(crate) struct RegistryDefaults {
    pub(crate) data_dir: PathBuf,
    pub(crate) hook_binary: PathBuf,
    pub(crate) copilot_binary: String,
    pub(crate) submit_gap: Duration,
    pub(crate) turn_timeout: Duration,
}

impl RegistryDefaults {
    /// Resolve sensible defaults. `hook_binary` is looked up as a sibling
    /// of the running daemon (works in both `cargo run` and the bundled
    /// `.app`).
    pub(crate) fn resolve(data_dir: PathBuf) -> Result<Self, AppError> {
        let exe = std::env::current_exe()
            .map_err(|e| AppError::Internal(format!("current_exe: {e}")))?;
        let hook_binary = exe
            .parent()
            .ok_or_else(|| AppError::Internal("exe has no parent".into()))?
            .join("bobe-copilot-hook");
        Ok(Self {
            data_dir,
            hook_binary,
            copilot_binary: "copilot".to_string(),
            submit_gap: Duration::from_millis(120),
            turn_timeout: Duration::from_mins(3),
        })
    }
}

/// A worker class. The registry uses the spec to lazy-spawn the matching
/// `CopilotWorker` on first `get`.
pub(crate) struct WorkerSpec {
    /// Stable class name, also the tmux session name. Examples:
    /// `bobe-goals`, `bobe-observe`, `bobe-vision`, `bobe-chat`,
    /// `bobe-consolidate`.
    pub(crate) name: String,
    /// Optional override for `turn_timeout` (e.g. consolidation needs longer).
    pub(crate) turn_timeout: Option<Duration>,
}

pub(crate) struct WorkerRegistry {
    defaults: RegistryDefaults,
    mux: Arc<Mux>,
    memory_file: Arc<MemoryFile>,
    workers: Mutex<HashMap<String, Arc<CopilotWorker>>>,
}

impl WorkerRegistry {
    pub(crate) fn new(defaults: RegistryDefaults, memory_file: Arc<MemoryFile>) -> Arc<Self> {
        let mux = Arc::new(Mux::new(defaults.submit_gap));
        Arc::new(Self {
            defaults,
            mux,
            memory_file,
            workers: Mutex::new(HashMap::new()),
        })
    }

    /// Lazy-spawn or return the worker for `spec.name`.
    pub(crate) async fn get_or_start(
        &self,
        spec: WorkerSpec,
    ) -> Result<Arc<dyn AgentWorker>, AppError> {
        // Fast path: existing worker.
        if let Some(w) = self.workers.lock().await.get(&spec.name).cloned() {
            return Ok(w);
        }

        // Slow path: spawn under lock so two concurrent callers don't
        // both spawn the same session.
        let mut guard = self.workers.lock().await;
        if let Some(w) = guard.get(&spec.name).cloned() {
            return Ok(w);
        }

        let worker_dir = self
            .defaults
            .data_dir
            .join("workers")
            .join(&spec.name);
        tokio::fs::create_dir_all(&worker_dir).await?;
        self.memory_file.symlink_into_worker(&worker_dir).await?;

        if !self.defaults.hook_binary.exists() {
            return Err(AppError::Internal(format!(
                "hook binary missing: {} — build with `cargo build --bin bobe-copilot-hook`",
                self.defaults.hook_binary.display()
            )));
        }

        let cfg = WorkerConfig {
            name: spec.name.clone(),
            dir: worker_dir,
            hook_binary: self.defaults.hook_binary.clone(),
            copilot_binary: self.defaults.copilot_binary.clone(),
            turn_timeout: spec.turn_timeout.unwrap_or(self.defaults.turn_timeout),
        };

        tracing::info!(name = %spec.name, "spawning copilot worker");
        let worker = CopilotWorker::start(cfg, Arc::clone(&self.mux))
            .await
            .map_err(|e| AppError::Internal(format!("spawn worker {}: {e}", spec.name)))?;

        guard.insert(spec.name.clone(), Arc::clone(&worker));
        Ok(worker)
    }

    pub(crate) async fn shutdown_all(&self) {
        let mut guard = self.workers.lock().await;
        for (name, worker) in guard.drain() {
            if let Err(e) = worker.shutdown().await {
                tracing::warn!(name = %name, err = %e, "worker shutdown failed");
            } else {
                tracing::info!(name = %name, "worker shut down");
            }
        }
    }
}
