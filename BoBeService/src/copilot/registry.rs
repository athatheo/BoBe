//! `WorkerRegistry` — owns the shared `ClientHandle` and the per-class
//! `Session`s.
//!
//! Each worker class has its own `Mutex<Option<...>>` for precise return
//! types (`Arc<BatchWorker>` for goals/decide/consolidate, `Arc<VisionWorker>`
//! for vision, `Arc<CopilotChatWorker>` for chat). Sessions are lazy: the
//! first call to a class accessor spawns the `Session` (after establishing
//! the shared CLI process via `ClientHandle::ensure_started`). Subsequent
//! calls return the cached `Arc`. `reload` drops every cached session +
//! stops the CLI so the next access re-spawns.
//!
//! Sessions persist across daemon restarts via `SessionStore`. Chat
//! rotates daily; everything else uses a stable per-class ID.
//!
//! ## Per-session BYOK
//!
//! `create_or_resume` builds each `SessionConfig` with class-appropriate
//! `model` + `provider` derived from the live `EngineConfig`:
//! - **Chat**   → `engine.provider_chat_model`
//! - **Vision** → `engine.provider_vision_model`
//! - **Goals / Decide / Consolidate** → `engine.provider_batch_model`
//!
//! In cloud mode (`engine == "copilot_cloud"`) only the `model` is set;
//! the CLI uses the signed-in user's GitHub Copilot endpoint. In local
//! mode the `provider` is also set (`{type:"openai", base_url:...}`),
//! pointing the CLI at Ollama.
//!
//! ## Hot-swap on engine change
//!
//! `ConfigManager::update` fires the engine-change listener after the
//! arc-swap `Config` is replaced. Bootstrap wires that listener to
//! `reload()`. The next worker access transparently re-spawns against
//! the new config — clients see no daemon restart.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use arc_swap::ArcSwap;
use chrono::{Local, NaiveDate};
use github_copilot_sdk::generated::api_types::{ModeSetRequest, SessionMode};
use github_copilot_sdk::session::Session;
use github_copilot_sdk::types::{
    InfiniteSessionConfig, McpServerConfig, ProviderConfig, ResumeSessionConfig, SessionConfig,
};
use tokio::sync::Mutex;

use crate::config::{Config, EngineConfig};
use crate::error::AppError;

use super::client::ClientHandle;
use super::handler::BobeHandler;
use super::hooks::BobeHooks;
use super::memory_file::MemoryFile;
use super::session_store::{CHAT_RETENTION_DAYS, SessionStore};
use super::types::WorkerClass;
use super::workers::batch::BatchWorker;
use super::workers::chat::CopilotChatWorker;
use super::workers::vision::VisionWorker;

/// Default Ollama OpenAI-compat endpoint. Used when `engine == "local"`
/// and `provider_base_url` is unset (typical post-wizard state once the
/// runtime is provisioned).
const DEFAULT_LOCAL_BASE_URL: &str = "http://127.0.0.1:11434/v1";

pub(crate) struct WorkerRegistry {
    client: Arc<ClientHandle>,
    config: Arc<ArcSwap<Config>>,
    session_store: SessionStore,
    memory_file: Arc<MemoryFile>,
    data_dir: PathBuf,
    /// MCP servers passed into every Copilot session via
    /// `SessionConfig::mcp_servers`. Loaded from `~/.bobe/mcp.json` at
    /// daemon start; changes still require restart to take effect (the
    /// SDK captures the map at session creation, and our sessions are
    /// long-lived).
    mcp_servers: HashMap<String, McpServerConfig>,

    goals: Mutex<Option<Arc<BatchWorker>>>,
    consolidate: Mutex<Option<Arc<BatchWorker>>>,
    decide: Mutex<Option<Arc<BatchWorker>>>,
    vision: Mutex<Option<Arc<VisionWorker>>>,
    /// Chat is keyed by local date so the cache invalidates at the
    /// midnight boundary. The mutex is contended only at session-spawn
    /// time, not on every `send`.
    chat: Mutex<Option<DatedChatWorker>>,
}

struct DatedChatWorker {
    date: NaiveDate,
    worker: Arc<CopilotChatWorker>,
}

impl WorkerRegistry {
    pub(crate) fn new(
        config: Arc<ArcSwap<Config>>,
        memory_file: Arc<MemoryFile>,
        data_dir: PathBuf,
        mcp_servers: HashMap<String, McpServerConfig>,
    ) -> Arc<Self> {
        Arc::new(Self {
            client: ClientHandle::new(Arc::clone(&config)),
            config,
            session_store: SessionStore::new(&data_dir),
            memory_file,
            data_dir,
            mcp_servers,
            goals: Mutex::new(None),
            consolidate: Mutex::new(None),
            decide: Mutex::new(None),
            vision: Mutex::new(None),
            chat: Mutex::new(None),
        })
    }

    /// Best-effort cleanup of chat session-id files older than
    /// `CHAT_RETENTION_DAYS`. For each one, we try to `destroy` the
    /// SDK-side session (so the upstream Copilot CLI releases its
    /// state too) and then delete the local pointer file. Failures
    /// are logged but never propagated — leftover files don't break
    /// anything, they just leak state.
    ///
    /// Called at boot from `bootstrap::run` after the registry is
    /// constructed but before any worker is spawned, so we don't
    /// race with the chat worker creating today's session.
    pub(crate) async fn prune_old_chat_sessions(&self) {
        let now = Local::now();
        let victims = match self
            .session_store
            .old_chat_sessions(now, CHAT_RETENTION_DAYS)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(err = %e, "registry.prune_chat.scan_failed");
                return;
            }
        };

        if victims.is_empty() {
            return;
        }

        let client = match self.client.ensure_started().await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(
                    err = %e,
                    count = victims.len(),
                    "registry.prune_chat.client_unavailable_skipping_destroy"
                );
                // Even without the client we can still nuke the local
                // files — the SDK-side state will simply linger until
                // the user clears it manually.
                for (path, _) in &victims {
                    drop(tokio::fs::remove_file(path).await);
                }
                return;
            }
        };

        let mut destroyed = 0usize;
        for (path, id) in victims {
            // Try to destroy the SDK-side session. If it's already
            // gone (NotFound), that's fine. If destroy fails, we
            // still remove the local pointer — there's nothing
            // useful for us to do with a stale id.
            let resume_cfg = github_copilot_sdk::types::ResumeSessionConfig::new(id.clone());
            if let Ok(session) = client.resume_session(resume_cfg).await
                && let Err(e) = session.destroy().await
            {
                tracing::debug!(
                    session_id = %id,
                    err = %e,
                    "registry.prune_chat.destroy_failed"
                );
            }
            if let Err(e) = tokio::fs::remove_file(&path).await {
                tracing::warn!(
                    path = %path.display(),
                    err = %e,
                    "registry.prune_chat.unlink_failed"
                );
            } else {
                destroyed += 1;
            }
        }

        if destroyed > 0 {
            tracing::info!(
                count = destroyed,
                retention_days = CHAT_RETENTION_DAYS,
                "registry.prune_chat.cleaned"
            );
        }
    }

    /// The single-writer memory.md handle this registry was constructed
    /// with. Consumers (capture learner, consolidation, memories
    /// handler) take an `Arc<MemoryFile>` from here rather than
    /// reaching back to bootstrap.
    pub(crate) fn memory_file(&self) -> Arc<MemoryFile> {
        Arc::clone(&self.memory_file)
    }

    pub(crate) async fn goals(&self) -> Result<Arc<BatchWorker>, AppError> {
        let mut guard = self.goals.lock().await;
        if let Some(w) = guard.as_ref() {
            return Ok(Arc::clone(w));
        }
        let session = self.create_or_resume(WorkerClass::Goals).await?;
        let worker = BatchWorker::new(WorkerClass::Goals, session);
        *guard = Some(Arc::clone(&worker));
        Ok(worker)
    }

    pub(crate) async fn consolidate(&self) -> Result<Arc<BatchWorker>, AppError> {
        let mut guard = self.consolidate.lock().await;
        if let Some(w) = guard.as_ref() {
            return Ok(Arc::clone(w));
        }
        let session = self.create_or_resume(WorkerClass::Consolidate).await?;
        let worker = BatchWorker::new(WorkerClass::Consolidate, session);
        *guard = Some(Arc::clone(&worker));
        Ok(worker)
    }

    pub(crate) async fn decide(&self) -> Result<Arc<BatchWorker>, AppError> {
        let mut guard = self.decide.lock().await;
        if let Some(w) = guard.as_ref() {
            return Ok(Arc::clone(w));
        }
        let session = self.create_or_resume(WorkerClass::Decide).await?;
        let worker = BatchWorker::new(WorkerClass::Decide, session);
        *guard = Some(Arc::clone(&worker));
        Ok(worker)
    }

    pub(crate) async fn vision(&self) -> Result<Arc<VisionWorker>, AppError> {
        let mut guard = self.vision.lock().await;
        if let Some(w) = guard.as_ref() {
            return Ok(Arc::clone(w));
        }
        let session = self.create_or_resume(WorkerClass::Vision).await?;
        let worker = VisionWorker::new(session);
        *guard = Some(Arc::clone(&worker));
        Ok(worker)
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

    /// Drop every cached session + stop the underlying Copilot CLI +
    /// forget every persisted session ID. Subsequent worker accessors
    /// transparently re-spawn against the current `Config` (which the
    /// caller is expected to have updated before invoking this).
    ///
    /// Called by `ConfigManager`'s engine-change listener wired in
    /// `bootstrap::run`. Forgetting session IDs is necessary because
    /// `ResumeSessionConfig` cannot override the `model` — a session
    /// created against `claude-sonnet-4` would resume with that model
    /// even if the engine flipped to local Ollama. Forgetting forces
    /// the fresh-create path, which honors the new model + provider.
    ///
    /// Best-effort: shutdown failures are logged but don't block the
    /// rebuild — leaving a stale session would be worse than a leak.
    pub(crate) async fn reload(&self) {
        // Drain caches first, before stopping the client, so any in-flight
        // worker access lands on the cleared cache and either waits on a
        // restart (if it grabbed the stale Arc earlier, that path is fine)
        // or re-spawns against the new client below.
        if let Some(w) = self.goals.lock().await.take() {
            log_shutdown("goals", w.shutdown().await);
        }
        if let Some(w) = self.consolidate.lock().await.take() {
            log_shutdown("consolidate", w.shutdown().await);
        }
        if let Some(w) = self.decide.lock().await.take() {
            log_shutdown("decide", w.shutdown().await);
        }
        if let Some(w) = self.vision.lock().await.take() {
            log_shutdown("vision", w.shutdown().await);
        }
        if let Some(dated) = self.chat.lock().await.take() {
            log_shutdown("chat", dated.worker.shutdown().await);
        }
        self.client.stop().await;

        // Forget on-disk session IDs so the next access creates fresh
        // sessions in the new engine.
        let now = Local::now();
        for class in [
            WorkerClass::Goals,
            WorkerClass::Consolidate,
            WorkerClass::Decide,
            WorkerClass::Vision,
            WorkerClass::Chat,
        ] {
            if let Err(e) = self.session_store.forget(class, now).await {
                tracing::warn!(class = %class.name(), err = %e, "registry.reload.forget_failed");
            }
        }

        tracing::info!("registry.reload_complete");
    }

    /// Best-effort shutdown of every spawned session, then the shared
    /// CLI process. Logged warnings only — daemon shutdown shouldn't
    /// fail because one worker's destroy errored.
    pub(crate) async fn shutdown_all(&self) {
        self.reload().await;
    }

    /// Build a `Session` for `class` — try resume from disk, fall back
    /// to creating a fresh one. Always installs `BobeHandler` (auto-
    /// approve permissions + observe usage) and `BobeHooks` (memory.md
    /// injection + structured error logging + per-turn context). Per-class
    /// model + provider come from the live `EngineConfig` snapshot.
    async fn create_or_resume(&self, class: WorkerClass) -> Result<Arc<Session>, AppError> {
        let client = self.client.ensure_started().await?;
        let now = Local::now();
        let handler = BobeHandler::new(class);
        let hooks = BobeHooks::new(class, Arc::clone(&self.memory_file));

        // Snapshot engine config once for this build. If it changes
        // mid-build, the next `reload()` will drop and re-spawn cleanly.
        let engine_snapshot = self.config.load().engine.clone();
        let (class_model, class_provider) = session_extras_for_class(&engine_snapshot, class);

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
            // Per-class model + provider from EngineConfig snapshot.
            if let Some(m) = class_model.clone() {
                cfg = cfg.with_model(m);
            }
            if let Some(p) = class_provider.clone() {
                cfg = cfg.with_provider(p);
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
            // `ResumeSessionConfig` exposes `provider` but not `model` — the
            // model is locked to whatever the session was created with. If
            // the engine config changed, `reload()` clears the session_store
            // so we land on the fresh-create path below instead of resuming
            // a model-locked session into a new provider.
            if let Some(p) = class_provider.clone() {
                resume_cfg.provider = Some(p);
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

/// Resolve the per-class `(model, provider)` overrides for `SessionConfig`.
///
/// - Chat / Decide / Goals / Consolidate / Vision each have a dedicated
///   model slot in `EngineConfig`.
/// - In cloud mode (`engine == "copilot_cloud"`) only the `model` is set;
///   the CLI uses the signed-in user's GitHub Copilot endpoint by default.
/// - In local mode (`engine == "local"`) we also build a `ProviderConfig`
///   pointing at the configured base URL (defaulting to `:11434/v1`),
///   provider type `"openai"`, no API key (Ollama doesn't require one).
fn session_extras_for_class(
    cfg: &EngineConfig,
    class: WorkerClass,
) -> (Option<String>, Option<ProviderConfig>) {
    let model = match class {
        WorkerClass::Chat => cfg.provider_chat_model.clone(),
        WorkerClass::Vision => cfg.provider_vision_model.clone(),
        WorkerClass::Goals | WorkerClass::Decide | WorkerClass::Consolidate => {
            cfg.provider_batch_model.clone()
        }
    };

    let provider = if cfg.engine == "local" {
        let base_url = cfg
            .provider_base_url
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_LOCAL_BASE_URL.to_string());
        let mut p = ProviderConfig::default();
        p.provider_type = Some("openai".to_string());
        p.base_url = base_url;
        Some(p)
    } else {
        None
    };

    (model, provider)
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
