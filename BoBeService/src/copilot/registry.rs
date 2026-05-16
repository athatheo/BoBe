//! Chat rotates daily; other worker classes use stable per-class session IDs.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

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
use super::hooks::{BobeHooks, HooksVoiceContext};
use super::memory_file::MemoryFile;
use super::session_store::SessionStore;
use super::types::WorkerClass;
use super::workers::batch::BatchWorker;
use super::workers::chat::CopilotChatWorker;
use super::workers::vision::VisionWorker;

use crate::constants::DEFAULT_OLLAMA_V1_URL as DEFAULT_LOCAL_BASE_URL;

pub(crate) struct WorkerRegistry {
    client: Arc<ClientHandle>,
    config: Arc<ArcSwap<Config>>,
    session_store: SessionStore,
    memory_file: Arc<MemoryFile>,
    data_dir: PathBuf,
    /// SDK captures this map at session creation; restart required to apply changes.
    mcp_servers: HashMap<String, McpServerConfig>,
    /// Voice deps used by BobeHooks at fire time. Bundled to keep
    /// BobeHooks::new from drilling 3 Arcs on every session create.
    voice: HooksVoiceContext,

    goals: Mutex<Option<Arc<BatchWorker>>>,
    consolidate: Mutex<Option<Arc<BatchWorker>>>,
    decide: Mutex<Option<Arc<BatchWorker>>>,
    vision: Mutex<Option<Arc<VisionWorker>>>,
    /// Date-keyed; cache invalidates at local midnight.
    chat: Mutex<Option<DatedChatWorker>>,
    /// Without this, back-to-back PATCHes fire concurrent reloads and `client.stop()` races mid-`create_session`.
    reload_lock: Mutex<()>,
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
        voice_turn_active: Arc<AtomicBool>,
        voice_sink: Arc<crate::voice::sinks::VoiceSink>,
        voice_engines: Arc<ArcSwap<crate::voice::engines::VoiceEnginesSnapshot>>,
    ) -> Arc<Self> {
        let client = ClientHandle::new(Arc::clone(&config));
        let voice = HooksVoiceContext {
            voice_turn_active,
            voice_sink,
            voice_engines,
        };
        Arc::new(Self {
            client,
            config,
            session_store: SessionStore::new(&data_dir),
            memory_file,
            data_dir,
            mcp_servers,
            voice,
            goals: Mutex::new(None),
            consolidate: Mutex::new(None),
            decide: Mutex::new(None),
            vision: Mutex::new(None),
            chat: Mutex::new(None),
            reload_lock: Mutex::new(()),
        })
    }

    pub(crate) fn memory_file(&self) -> Arc<MemoryFile> {
        Arc::clone(&self.memory_file)
    }

    pub(crate) fn client_handle(&self) -> Arc<ClientHandle> {
        Arc::clone(&self.client)
    }

    /// Returns `None` (not empty Vec) when chat is unspawned, signaling "indeterminate" to UI.
    /// `session.mcp.list` is marked experimental by the SDK; pin versions.
    pub(crate) async fn live_mcp_servers(
        &self,
    ) -> Option<Vec<github_copilot_sdk::generated::api_types::McpServer>> {
        let session = {
            let guard = self.chat.lock().await;
            let dated = guard.as_ref()?;
            dated.worker.session()
        };
        match session.rpc().mcp().list().await {
            Ok(list) => Some(list.servers),
            Err(e) => {
                tracing::warn!(err = %e, "registry.live_mcp_servers.rpc_failed");
                Some(Vec::new())
            }
        }
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
        let today = Local::now().date_naive();
        let mut guard = self.chat.lock().await;

        if let Some(existing) = guard.as_ref()
            && existing.date == today
        {
            return Ok(Arc::clone(&existing.worker));
        }

        if let Some(stale) = guard.take() {
            tracing::info!(
                old_date = %stale.date,
                new_date = %today,
                "chat worker rotating across local-date boundary"
            );
            // disconnect (not destroy) preserves prior day's history on disk.
            if let Err(e) = stale.worker.shutdown().await {
                tracing::warn!(err = %e, "chat rotation: stale shutdown failed");
            }
        }

        let engine_snapshot = self.config.load().engine.clone();
        let (chat_model, _) = session_extras_for_class(&engine_snapshot, WorkerClass::Chat);
        let session = self.create_or_resume(WorkerClass::Chat).await?;
        let worker = CopilotChatWorker::new(session, chat_model);
        *guard = Some(DatedChatWorker {
            date: today,
            worker: Arc::clone(&worker),
        });
        Ok(worker)
    }

    /// Forgets session IDs because `ResumeSessionConfig` cannot override `model` — forces fresh-create.
    pub(crate) async fn reload(&self) {
        let _reload_guard = self.reload_lock.lock().await;

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

    /// Soft reload — model/reasoning fields changed but the SDK process and
    /// chat session can be preserved. Today this just forwards to `reload()`
    /// for safety; the chat session stays alive because `reload()` will
    /// recreate it with the new model on next access via `create_or_resume`.
    /// Pivot's smarter soft-reload (chat session preserved literally) is
    /// post-merge work tracked in `services::model_resolver`.
    pub(crate) async fn reload_soft(&self) {
        self.reload().await;
    }

    pub(crate) async fn shutdown_all(&self) {
        self.reload().await;
    }

    async fn create_or_resume(&self, class: WorkerClass) -> Result<Arc<Session>, AppError> {
        let client = self.client.ensure_started().await?;
        let now = Local::now();
        let handler = BobeHandler::new(class);
        let hooks = BobeHooks::new(class, Arc::clone(&self.memory_file), self.voice.clone());

        let engine_snapshot = self.config.load().engine.clone();
        let (class_model, class_provider) = session_extras_for_class(&engine_snapshot, class);

        // Session mode set at runtime via `apply_runtime_mode`, not via `SessionConfig`.
        let cfg_template = || {
            let mut cfg = SessionConfig::default()
                .with_handler(Arc::clone(&handler) as _)
                .with_hooks(Arc::clone(&hooks) as _);
            cfg.streaming = Some(class == WorkerClass::Chat);
            cfg.infinite_sessions = Some(InfiniteSessionConfig::new());
            if let Some(skill_dir) = self.skill_dir(class) {
                cfg.skill_directories = Some(vec![skill_dir]);
            }
            if class == WorkerClass::Chat && !self.mcp_servers.is_empty() {
                cfg.mcp_servers = Some(self.mcp_servers.clone());
            }
            // Autopilot classes have no user; `ask_user` would deadlock them.
            if class != WorkerClass::Chat {
                cfg = cfg.with_request_user_input(false);
            }
            if let Some(m) = class_model.clone() {
                cfg = cfg.with_model(m);
            }
            if let Some(p) = class_provider.clone() {
                cfg = cfg.with_provider(p);
            }
            cfg
        };

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
            // Session params don't carry across disconnect/resume — must re-set.
            if class != WorkerClass::Chat {
                resume_cfg.request_user_input = Some(false);
            }
            // `ResumeSessionConfig` lacks `model`; engine change clears store and routes to fresh-create.
            if let Some(p) = class_provider.clone() {
                resume_cfg.provider = Some(p);
            }
            match client.resume_session(resume_cfg).await {
                Ok(session) => {
                    apply_runtime_mode(&session, class).await?;
                    // CLI may reassign session id on resume (per SDK docstring); re-save to avoid stale id.
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

    fn skill_dir(&self, class: WorkerClass) -> Option<PathBuf> {
        let p = self.data_dir.join("skills").join(class.name());
        if p.exists() { Some(p) } else { None }
    }
}

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

    let provider = if cfg.engine == crate::constants::engine_kind::LOCAL {
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

/// Non-chat mode failures are fatal: autopilot drives batch auto-loop; without it `send_and_wait` hangs.
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
