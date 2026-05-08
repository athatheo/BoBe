//! `CopilotWorker` — one tmux session, one Copilot CLI process, one
//! inbox/outbox pair, one hook socket. Submits jobs serially; the hook
//! `notification` event signals "turn done; outbox has the result."

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

use super::hook::{HookRouter, ListenerHandle, spawn_listener};
use super::hook_install;
use super::mux::Mux;

/// Wait at most this long for `notification` after `send_keys`. Configurable
/// per-worker if a class typically takes longer (consolidation, vision).
#[allow(dead_code, reason = "Phase 1: callers pick their own timeout; Phase 2 will default through this")]
pub(crate) const DEFAULT_TURN_TIMEOUT: Duration = Duration::from_mins(2);

#[derive(Debug, thiserror::Error)]
pub(crate) enum WorkerError {
    #[error("tmux: {0}")]
    Mux(#[from] super::mux::MuxError),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("turn timed out after {}s", .0.as_secs())]
    Timeout(Duration),

    #[error("worker outbox missing for job {0}")]
    OutboxMissing(Uuid),
}

/// What goes into `inbox/<id>.json`. Free-form `input` lets each worker
/// class keep its own schema; the worker just promises to process it and
/// emit `outbox/<id>.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct JobInput {
    pub(crate) job_id: Uuid,
    pub(crate) kind: String,
    pub(crate) instructions: String,
    pub(crate) input: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct JobOutput {
    pub(crate) job_id: Uuid,
    #[serde(default)]
    pub(crate) output: serde_json::Value,
    #[serde(default)]
    pub(crate) error: Option<String>,
}

/// Long-lived worker. Construct with `start`, submit work with `submit`,
/// stop with `shutdown` (kills tmux session AND aborts the hook listener
/// task — without that, restarting a worker leaks the prior listener).
pub(crate) struct CopilotWorker {
    name: String,
    dir: PathBuf,
    mux: Arc<Mux>,
    router: Arc<HookRouter>,
    /// Serializes job submission per worker — only one in-flight at a time.
    submit_lock: Mutex<()>,
    turn_timeout: Duration,
    /// Abort handle for the per-worker UDS listener. `Mutex<Option<...>>`
    /// because `shutdown` consumes the handle and we still want `&self`.
    listener: Mutex<Option<ListenerHandle>>,
}

pub(crate) struct WorkerConfig {
    pub(crate) name: String,
    pub(crate) dir: PathBuf,
    pub(crate) hook_binary: PathBuf,
    pub(crate) copilot_binary: String,
    pub(crate) turn_timeout: Duration,
}

impl CopilotWorker {
    /// Bind the hook socket, install Copilot's hook config, spawn the
    /// tmux session running `copilot --allow-all-tools`. Idempotent on
    /// the tmux side: if the session already exists, reuse it.
    pub(crate) async fn start(cfg: WorkerConfig, mux: Arc<Mux>) -> Result<Arc<Self>, WorkerError> {
        std::fs::create_dir_all(cfg.dir.join("inbox"))?;
        std::fs::create_dir_all(cfg.dir.join("outbox"))?;

        // Per-worker hook config + socket inside the worker dir.
        hook_install::install(&cfg.dir, &cfg.hook_binary)?;
        let socket_path = cfg.dir.join("hook.sock");

        let router = HookRouter::new();
        let listener = spawn_listener(socket_path.clone(), Arc::clone(&router)).await?;

        // Spawn the tmux session if not already running.
        if !mux.has_session(&cfg.name).await? {
            let socket_str = socket_path.to_string_lossy().to_string();
            let env: Vec<(&str, &str)> = vec![("BOBE_HOOK_SOCKET", &socket_str)];
            // Copilot CLI v1.x flags: --allow-all-tools to skip permission
            // prompts. If the flag changes, this is the single point to
            // fix.
            let cmd: Vec<&str> = vec![&cfg.copilot_binary, "--allow-all-tools"];
            mux.new_session(&cfg.name, &cfg.dir, &env, &cmd).await?;
            // Give Copilot a moment to render its prompt before we send
            // input. Tunable; if Copilot is slow to boot, raise this.
            tokio::time::sleep(Duration::from_millis(800)).await;
        }

        Ok(Arc::new(Self {
            name: cfg.name,
            dir: cfg.dir,
            mux,
            router,
            submit_lock: Mutex::new(()),
            turn_timeout: cfg.turn_timeout,
            listener: Mutex::new(Some(listener)),
        }))
    }

    /// Submit one job, block until Copilot signals completion via the
    /// hook, return the parsed outbox file.
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) async fn submit(&self, job: JobInput) -> Result<JobOutput, WorkerError> {
        let _guard = self.submit_lock.lock().await;

        let inbox_path = self.dir.join("inbox").join(format!("{}.json", job.job_id));
        let outbox_path = self.dir.join("outbox").join(format!("{}.json", job.job_id));

        let body = serde_json::to_vec_pretty(&job)?;
        std::fs::write(&inbox_path, body)?;

        let rx = self.router.register(job.job_id).await;

        let prompt = format!(
            "Read inbox/{0}.json. Follow the `instructions` field. Write the result \
             as JSON to outbox/{0}.json with shape {{\"job_id\":\"{0}\",\"output\":<...>}}. \
             Do not ask questions. When finished, exit your turn so I am notified.",
            job.job_id
        );

        if let Err(e) = self.mux.send_keys(&self.name, &prompt).await {
            self.router.cancel(job.job_id).await;
            return Err(e.into());
        }

        // `Ok(Ok(_))` is delivery; recv-closed and outer-timeout both
        // surface as Timeout — Copilot didn't signal in time.
        let Ok(Ok(env)) = tokio::time::timeout(self.turn_timeout, rx).await else {
            self.router.cancel(job.job_id).await;
            return Err(WorkerError::Timeout(self.turn_timeout));
        };

        tracing::debug!(
            worker = %self.name,
            event = %env.event,
            job = %job.job_id,
            "worker received completion notification"
        );

        if !outbox_path.exists() {
            return Err(WorkerError::OutboxMissing(job.job_id));
        }
        let raw = std::fs::read(&outbox_path)?;
        let out: JobOutput = serde_json::from_slice(&raw)?;
        Ok(out)
    }

    #[allow(dead_code, reason = "Phase 1: caller wiring lands in Phase 2")]
    pub(crate) async fn shutdown(&self) -> Result<(), WorkerError> {
        // Order: kill tmux first so Copilot CLI stops invoking hooks,
        // then abort the listener so its socket is released cleanly.
        self.mux.kill_session(&self.name).await?;
        if let Some(listener) = self.listener.lock().await.take() {
            listener.shutdown();
        }
        Ok(())
    }
}
