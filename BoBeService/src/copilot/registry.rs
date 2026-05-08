//! `WorkerRegistry` — owns one `github_copilot_sdk::Client` (= one Copilot
//! CLI server process) and lazy-spawns `CopilotWorker`s (one `Session`
//! each) keyed by class name. `shutdown_all` closes every session and
//! stops the client.
//!
//! Memory injection: on every session creation we register an
//! `on_session_start` hook that reads the current `memory.md` body and
//! returns it as `additional_context`. Workers see pruned memory at the
//! start of every turn — no symlinks, no file watching.

#![allow(
    dead_code,
    reason = "Phase 2: registry + spec types; consumers cut over in Phase 5"
)]

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use github_copilot_sdk::handler::ApproveAllHandler;
use github_copilot_sdk::hooks::{HookEvent, HookOutput, SessionHooks, SessionStartOutput};
use github_copilot_sdk::types::SessionConfig;
use github_copilot_sdk::{Client, ClientOptions};
use tokio::sync::{Mutex, OnceCell};

use crate::error::AppError;

use super::agent_worker::AgentWorker;
use super::memory_file::MemoryFile;
use super::worker::{CopilotWorker, WorkerConfig};

/// Per-class config. Currently only the timeout differs — the SDK takes
/// care of model selection/auth/etc.
pub(crate) struct WorkerSpec {
    /// Stable worker class name (also session label for tracing).
    pub(crate) name: String,
    pub(crate) turn_timeout: Option<Duration>,
}

pub(crate) struct WorkerRegistry {
    /// Lazily-started SDK client. Spawning the CLI is expensive (~hundreds
    /// of ms), so we defer it until the first worker is requested.
    client: OnceCell<Arc<Client>>,
    memory_file: Arc<MemoryFile>,
    workers: Mutex<HashMap<String, Arc<CopilotWorker>>>,
    default_turn_timeout: Duration,
}

impl WorkerRegistry {
    pub(crate) fn new(memory_file: Arc<MemoryFile>) -> Arc<Self> {
        Arc::new(Self {
            client: OnceCell::new(),
            memory_file,
            workers: Mutex::new(HashMap::new()),
            default_turn_timeout: super::worker::DEFAULT_TURN_TIMEOUT,
        })
    }

    /// Look up or spawn the worker for `spec.name`.
    pub(crate) async fn get_or_start(
        &self,
        spec: WorkerSpec,
    ) -> Result<Arc<dyn AgentWorker>, AppError> {
        if let Some(w) = self.workers.lock().await.get(&spec.name).cloned() {
            return Ok(w);
        }

        let client = self.ensure_client().await?;

        let mut guard = self.workers.lock().await;
        if let Some(w) = guard.get(&spec.name).cloned() {
            return Ok(w);
        }

        let hooks: Arc<dyn SessionHooks> = Arc::new(MemoryHooks {
            memory_file: Arc::clone(&self.memory_file),
        });

        // ApproveAllHandler: built-in handler that approves every
        // permission request and uses safe defaults for the rest
        // (no-op user input, deny external tools, cancel elicitation).
        // Sandbox-as-a-feature: workers run inside ~/.bobe/, so we don't
        // gate tool use.
        let cfg = SessionConfig::default()
            .with_handler(Arc::new(ApproveAllHandler))
            .with_hooks(hooks);

        tracing::info!(name = %spec.name, "spawning copilot SDK session");
        let session = client
            .create_session(cfg)
            .await
            .map_err(|e| AppError::Internal(format!("create session {}: {e}", spec.name)))?;

        let worker = CopilotWorker::new(
            Arc::new(session),
            WorkerConfig {
                name: spec.name.clone(),
                turn_timeout: spec.turn_timeout.unwrap_or(self.default_turn_timeout),
            },
        );

        guard.insert(spec.name.clone(), Arc::clone(&worker));
        Ok(worker)
    }

    async fn ensure_client(&self) -> Result<Arc<Client>, AppError> {
        self.client
            .get_or_try_init(|| async {
                tracing::info!("starting Copilot SDK client");
                let client = Client::start(ClientOptions::default())
                    .await
                    .map_err(|e| AppError::Internal(format!("Client::start: {e}")))?;
                Ok::<_, AppError>(Arc::new(client))
            })
            .await
            .cloned()
    }

    pub(crate) async fn shutdown_all(&self) {
        let drained: Vec<(String, Arc<CopilotWorker>)> = {
            let mut guard = self.workers.lock().await;
            guard.drain().collect()
        };
        for (name, worker) in drained {
            if let Err(e) = worker.shutdown().await {
                tracing::warn!(name = %name, err = %e, "worker shutdown failed");
            } else {
                tracing::info!(name = %name, "worker shut down");
            }
        }

        if let Some(client) = self.client.get() {
            if let Err(e) = client.stop().await {
                tracing::warn!(err = %e, "copilot client stop failed");
            } else {
                tracing::info!("copilot client stopped");
            }
        }
    }
}

/// Reads memory.md on every `on_session_start` and emits it as
/// `additional_context`. Cheap — memory.md is capped at ~50 KB by the
/// nightly consolidation worker.
struct MemoryHooks {
    memory_file: Arc<MemoryFile>,
}

#[async_trait]
impl SessionHooks for MemoryHooks {
    async fn on_hook(&self, event: HookEvent) -> HookOutput {
        if let HookEvent::SessionStart { ctx, .. } = event {
            match self.memory_file.read().await {
                Ok(body) => {
                    tracing::debug!(
                        session = %ctx.session_id,
                        bytes = body.len(),
                        "injecting memory.md as session context"
                    );
                    return HookOutput::SessionStart(SessionStartOutput {
                        additional_context: Some(body),
                        ..Default::default()
                    });
                }
                Err(e) => {
                    tracing::warn!(err = %e, "memory_file.read failed; session starts without context");
                }
            }
        }
        HookOutput::None
    }
}
