//! `WorkerRegistry` — owns the shared `ClientHandle`, the per-class
//! `Session`s, and the cross-cutting observability (`UsageMeter`).

#![allow(
    dead_code,
    reason = "Phase 6: per-class accessors complete; Phase 5 wires consumers"
)]
//!
//! Each worker class has its own `OnceCell` so types are precise:
//!
//! ```text
//!   registry.goals().await       -> Arc<BatchWorker>
//!   registry.observe().await     -> Arc<BatchWorker>
//!   registry.consolidate().await -> Arc<BatchWorker>
//!   registry.vision().await      -> Arc<VisionWorker>
//!   registry.chat().await        -> Arc<CopilotChatWorker>
//! ```
//!
//! Sessions are lazy: the first call to a class accessor spawns the
//! `Session` (after establishing the shared CLI process via
//! `ClientHandle::ensure_started`). Subsequent calls return the cached
//! `Arc`. Shutdown is one `shutdown_all` that walks every spawned class.
//!
//! Sessions persist across daemon restarts via `SessionStore`. Chat
//! rotates daily; everything else uses a stable per-class ID.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::{Local, NaiveDate};
use github_copilot_sdk::generated::api_types::{ModeSetRequest, SessionMode};
use github_copilot_sdk::session::Session;
use github_copilot_sdk::types::{
    InfiniteSessionConfig, McpServerConfig, ResumeSessionConfig, SessionConfig,
};
use tokio::sync::{Mutex, OnceCell};

use crate::error::AppError;

use super::client::ClientHandle;
use super::handler::BobeHandler;
use super::hooks::BobeHooks;
use super::memory_file::MemoryFile;
use super::session_store::SessionStore;
use super::types::WorkerClass;
use super::usage::UsageMeter;
use super::workers::batch::BatchWorker;
use super::workers::chat::CopilotChatWorker;
use super::workers::vision::VisionWorker;

pub(crate) struct WorkerRegistry {
    client: Arc<ClientHandle>,
    session_store: SessionStore,
    memory_file: Arc<MemoryFile>,
    usage: Arc<UsageMeter>,
    data_dir: PathBuf,
    /// MCP servers passed into every Copilot session via
    /// `SessionConfig::mcp_servers`. Loaded from `~/.bobe/mcp.json` at
    /// daemon start; changes require restart to take effect (the SDK
    /// captures the map at session creation, and our sessions are
    /// long-lived).
    mcp_servers: HashMap<String, McpServerConfig>,

    goals: OnceCell<Arc<BatchWorker>>,
    observe: OnceCell<Arc<BatchWorker>>,
    consolidate: OnceCell<Arc<BatchWorker>>,
    decide: OnceCell<Arc<BatchWorker>>,
    vision: OnceCell<Arc<VisionWorker>>,
    /// Chat is keyed by local date so the cache invalidates at the
    /// midnight boundary — a `OnceCell` would pin the first day's
    /// session forever. The mutex is contended only at session-spawn
    /// time, not on every `send`.
    chat: Mutex<Option<DatedChatWorker>>,
}

struct DatedChatWorker {
    date: NaiveDate,
    worker: Arc<CopilotChatWorker>,
}

impl WorkerRegistry {
    pub(crate) fn new(
        memory_file: Arc<MemoryFile>,
        data_dir: PathBuf,
        mcp_servers: HashMap<String, McpServerConfig>,
    ) -> Arc<Self> {
        Arc::new(Self {
            client: ClientHandle::new(),
            session_store: SessionStore::new(&data_dir),
            memory_file,
            usage: UsageMeter::new(),
            data_dir,
            mcp_servers,
            goals: OnceCell::new(),
            observe: OnceCell::new(),
            consolidate: OnceCell::new(),
            decide: OnceCell::new(),
            vision: OnceCell::new(),
            chat: Mutex::new(None),
        })
    }

    pub(crate) fn usage_meter(&self) -> &Arc<UsageMeter> {
        &self.usage
    }

    /// The single-writer memory.md handle this registry was constructed
    /// with. Consumers (capture learner, consolidation, memories
    /// handler) take an `Arc<MemoryFile>` from here rather than
    /// reaching back to bootstrap.
    pub(crate) fn memory_file(&self) -> Arc<MemoryFile> {
        Arc::clone(&self.memory_file)
    }

    pub(crate) async fn goals(&self) -> Result<Arc<BatchWorker>, AppError> {
        self.goals
            .get_or_try_init(|| async {
                let session = self.create_or_resume(WorkerClass::Goals).await?;
                Ok::<_, AppError>(BatchWorker::new(WorkerClass::Goals, session))
            })
            .await
            .cloned()
    }

    pub(crate) async fn observe(&self) -> Result<Arc<BatchWorker>, AppError> {
        self.observe
            .get_or_try_init(|| async {
                let session = self.create_or_resume(WorkerClass::Observe).await?;
                Ok::<_, AppError>(BatchWorker::new(WorkerClass::Observe, session))
            })
            .await
            .cloned()
    }

    pub(crate) async fn consolidate(&self) -> Result<Arc<BatchWorker>, AppError> {
        self.consolidate
            .get_or_try_init(|| async {
                let session = self.create_or_resume(WorkerClass::Consolidate).await?;
                Ok::<_, AppError>(BatchWorker::new(WorkerClass::Consolidate, session))
            })
            .await
            .cloned()
    }

    pub(crate) async fn decide(&self) -> Result<Arc<BatchWorker>, AppError> {
        self.decide
            .get_or_try_init(|| async {
                let session = self.create_or_resume(WorkerClass::Decide).await?;
                Ok::<_, AppError>(BatchWorker::new(WorkerClass::Decide, session))
            })
            .await
            .cloned()
    }

    pub(crate) async fn vision(&self) -> Result<Arc<VisionWorker>, AppError> {
        self.vision
            .get_or_try_init(|| async {
                let session = self.create_or_resume(WorkerClass::Vision).await?;
                Ok::<_, AppError>(VisionWorker::new(session))
            })
            .await
            .cloned()
    }

    pub(crate) async fn chat(&self) -> Result<Arc<CopilotChatWorker>, AppError> {
        // Date-keyed cache: at local midnight the previous day's worker
        // is dropped (its `Drop` calls `disconnect`, preserving on-disk
        // state) and a new one is spawned against today's persistent ID.
        let today = Local::now().date_naive();
        let mut guard = self.chat.lock().await;

        if let Some(existing) = guard.as_ref()
            && existing.date == today
        {
            return Ok(Arc::clone(&existing.worker));
        }

        // Either no chat worker, or yesterday's. Spawn fresh for today.
        if let Some(stale) = guard.take() {
            tracing::info!(
                old_date = %stale.date,
                new_date = %today,
                "chat worker rotating across local-date boundary"
            );
            // Best-effort disconnect of yesterday's session so it
            // releases its idle_waiter slot and the SDK can clean up
            // promptly. The on-disk session state is preserved by
            // disconnect (vs destroy), so the prior day's history is
            // still queryable via the SDK if we ever want to.
            if let Err(e) = stale.worker.shutdown().await {
                tracing::warn!(err = %e, "chat rotation: stale shutdown failed");
            }
        }

        let session = self.create_or_resume(WorkerClass::Chat).await?;
        let worker = CopilotChatWorker::new(session);
        *guard = Some(DatedChatWorker {
            date: today,
            worker: Arc::clone(&worker),
        });
        Ok(worker)
    }

    /// Best-effort shutdown of every spawned session, then the shared
    /// CLI process. Logged warnings only — daemon shutdown shouldn't
    /// fail because one worker's destroy errored.
    pub(crate) async fn shutdown_all(&self) {
        if let Some(w) = self.goals.get() {
            log_shutdown("goals", w.shutdown().await);
        }
        if let Some(w) = self.observe.get() {
            log_shutdown("observe", w.shutdown().await);
        }
        if let Some(w) = self.consolidate.get() {
            log_shutdown("consolidate", w.shutdown().await);
        }
        if let Some(w) = self.decide.get() {
            log_shutdown("decide", w.shutdown().await);
        }
        if let Some(w) = self.vision.get() {
            log_shutdown("vision", w.shutdown().await);
        }
        if let Some(dated) = self.chat.lock().await.take() {
            log_shutdown("chat", dated.worker.shutdown().await);
        }
        self.client.stop().await;
    }

    /// Build a `Session` for `class` — try resume from disk, fall back
    /// to creating a fresh one. Always installs `BobeHandler` (auto-
    /// approve permissions + observe usage) and `BobeHooks` (memory.md
    /// injection + structured error logging + per-turn context).
    async fn create_or_resume(&self, class: WorkerClass) -> Result<Arc<Session>, AppError> {
        let client = self.client.ensure_started().await?;
        let now = Local::now();
        let handler = BobeHandler::new(class, Arc::clone(&self.usage));
        let hooks = BobeHooks::new(class, Arc::clone(&self.memory_file));

        // Session mode (autopilot/interactive/plan) isn't a `SessionConfig`
        // field — it's set at runtime via `session.rpc().mode().set(...)`
        // once the session is up. `apply_runtime_mode` handles that below.
        let cfg_template = || {
            let mut cfg = SessionConfig::default()
                .with_handler(Arc::clone(&handler) as _)
                .with_hooks(Arc::clone(&hooks) as _);
            cfg.streaming = Some(class == WorkerClass::Chat);
            // Auto-compaction on every class. Chat needs it because
            // user dialogue grows unboundedly within a day. Batch
            // classes need it because their sessions persist across
            // daemon restarts and the Goals worker in particular is
            // shared across goal-extraction, agent-job evaluation,
            // and conversation summary jobs — without compaction the
            // history grows monotonically over weeks.
            cfg.infinite_sessions = Some(InfiniteSessionConfig::new());
            if let Some(skill_dir) = self.skill_dir(class) {
                cfg.skill_directories = Some(vec![skill_dir]);
            }
            // MCP tools belong to interactive chat — batch classes
            // have narrow autopilot jobs and don't need extra tools.
            if class == WorkerClass::Chat && !self.mcp_servers.is_empty() {
                cfg.mcp_servers = Some(self.mcp_servers.clone());
            }
            cfg
        };

        // Try resume from disk first.
        if let Some(saved) = self.session_store.load(class, now).await? {
            let mut resume_cfg = ResumeSessionConfig::new(saved.clone())
                .with_handler(Arc::clone(&handler) as _)
                .with_hooks(Arc::clone(&hooks) as _);
            resume_cfg.streaming = Some(class == WorkerClass::Chat);
            resume_cfg.infinite_sessions = Some(InfiniteSessionConfig::new());
            if let Some(skill_dir) = self.skill_dir(class) {
                resume_cfg.skill_directories = Some(vec![skill_dir]);
            }
            if class == WorkerClass::Chat && !self.mcp_servers.is_empty() {
                resume_cfg.mcp_servers = Some(self.mcp_servers.clone());
            }
            match client.resume_session(resume_cfg).await {
                Ok(session) => {
                    apply_runtime_mode(&session, class).await?;
                    // The CLI may reassign the session id on resume (per the
                    // SDK docstring on `Session::resume`). If so, re-save so
                    // the next daemon start resumes the *current* id rather
                    // than a stale one — otherwise resume silently fails
                    // and we lose accumulated context every restart.
                    let live_id = session.id().clone();
                    if live_id != saved {
                        tracing::info!(
                            class = %class.name(),
                            old_id = %saved,
                            new_id = %live_id,
                            "CLI reassigned session id on resume; updating store"
                        );
                        self.session_store.save(class, now, &live_id).await?;
                    }
                    tracing::info!(
                        class = %class.name(),
                        session_id = %live_id,
                        "resumed copilot session from disk"
                    );
                    return Ok(Arc::new(session));
                }
                Err(e) => {
                    tracing::warn!(
                        class = %class.name(),
                        session_id = %saved,
                        err = %e,
                        "resume failed; creating fresh session"
                    );
                    self.session_store.forget(class, now).await?;
                }
            }
        }

        // Create fresh.
        let session = client
            .create_session(cfg_template())
            .await
            .map_err(|e| AppError::Internal(format!("create_session {}: {e}", class.name())))?;
        apply_runtime_mode(&session, class).await?;
        let id = session.id().clone();
        self.session_store.save(class, now, &id).await?;
        tracing::info!(
            class = %class.name(),
            session_id = %id,
            "created new copilot session"
        );
        Ok(Arc::new(session))
    }

    /// Path to `~/.bobe/skills/<class>/` if the directory exists. Loaded
    /// into `SessionConfig::skill_directories` so each worker sees its
    /// own `SKILL.md` as system context — the stable identity for the
    /// class, complementing per-job `instructions`.
    fn skill_dir(&self, class: WorkerClass) -> Option<PathBuf> {
        let p = self.data_dir.join("skills").join(class.name());
        if p.exists() { Some(p) } else { None }
    }
}

fn log_shutdown(class: &str, result: Result<(), super::error::WorkerError>) {
    match result {
        Ok(()) => tracing::info!(class, "worker shut down"),
        Err(e) => tracing::warn!(class, err = %e, "worker shutdown failed"),
    }
}

/// Apply the worker class's mode (autopilot / interactive / plan) to a
/// freshly-created or resumed `Session`. Wire method: `session.mode.set`.
///
/// **Failure policy:** for non-chat classes, mode mismatch is functionally
/// fatal — autopilot is what makes batch jobs auto-loop to
/// `task_complete`; without it `send_and_wait` hangs until the per-class
/// turn timeout (3-15 minutes). We propagate the error so consumers see
/// it immediately. Chat's default mode is interactive anyway, so a
/// failed `set("interactive")` is recoverable; we log + continue.
async fn apply_runtime_mode(session: &Session, class: WorkerClass) -> Result<(), AppError> {
    let mode = match class.mode() {
        "interactive" => SessionMode::Interactive,
        "plan" => SessionMode::Plan,
        "autopilot" => SessionMode::Autopilot,
        other => {
            tracing::warn!(
                class = %class.name(),
                mode = other,
                "unknown session mode; leaving session at default"
            );
            return Ok(());
        }
    };
    match session.rpc().mode().set(ModeSetRequest { mode }).await {
        Ok(()) => Ok(()),
        Err(e) => {
            if class == WorkerClass::Chat {
                tracing::warn!(
                    class = %class.name(),
                    err = %e,
                    "chat session.mode.set failed; default mode is interactive — continuing"
                );
                Ok(())
            } else {
                Err(AppError::Internal(format!(
                    "session.mode.set({}) failed for {}: {e} — batch classes need autopilot",
                    class.mode(),
                    class.name()
                )))
            }
        }
    }
}
