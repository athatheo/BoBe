//! Chat rotates daily; other worker classes use stable per-class session IDs.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use arc_swap::ArcSwap;
use chrono::{Local, NaiveDate};
use github_copilot_sdk::IndexMap;
use github_copilot_sdk::mode::{BUILTIN_TOOLS_ISOLATED, ToolSet};
use github_copilot_sdk::session::Session;
use github_copilot_sdk::types::{
    InfiniteSessionConfig, LargeToolOutputConfig, McpServerConfig, MemoryConfiguration,
    ResumeSessionConfig, SessionConfig, SessionLimitsConfig,
};
use tokio::sync::{Mutex, RwLock};

use crate::config::Config;
use crate::error::AppError;

use super::client::ClientHandle;
use super::handler::BobeHandler;
use super::hooks::{BobeHooks, HooksVoiceContext};
use super::memory_file::MemoryFile;
use super::registry_session_factory::{apply_runtime_mode, log_shutdown, session_extras_for_class};
use super::session_store::SessionStore;
use super::types::WorkerClass;
use super::workers::batch::BatchWorker;
use super::workers::chat::CopilotChatWorker;
use super::workers::vision::VisionWorker;

pub(crate) struct WorkerRegistry {
    client: Arc<ClientHandle>,
    config: Arc<ArcSwap<Config>>,
    session_store: SessionStore,
    memory_file: Arc<MemoryFile>,
    data_dir: PathBuf,
    mcp: RwLock<McpRuntimeConfig>,
    /// Voice deps used by BobeHooks at fire time. Bundled to keep
    /// BobeHooks::new from drilling 3 Arcs on every session create.
    voice: HooksVoiceContext,

    goals: Mutex<Option<Arc<BatchWorker>>>,
    consolidate: Mutex<Option<Arc<BatchWorker>>>,
    vision: Mutex<Option<Arc<VisionWorker>>>,
    /// Date-keyed; cache invalidates at local midnight.
    chat: Mutex<Option<DatedChatWorker>>,
    /// Without this, back-to-back PATCHes fire concurrent reloads and `client.stop()` races mid-`create_session`.
    lifecycle: Arc<RwLock<()>>,
}

struct McpRuntimeConfig {
    servers: IndexMap<String, McpServerConfig>,
    excluded_tools: Vec<String>,
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
        mcp_servers: IndexMap<String, McpServerConfig>,
        excluded_tools: Vec<String>,
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
            mcp: RwLock::new(McpRuntimeConfig {
                servers: mcp_servers,
                excluded_tools,
            }),
            voice,
            goals: Mutex::new(None),
            consolidate: Mutex::new(None),
            vision: Mutex::new(None),
            chat: Mutex::new(None),
            lifecycle: Arc::new(RwLock::new(())),
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
    pub(crate) async fn live_mcp_servers(&self) -> Option<Vec<github_copilot_sdk::rpc::McpServer>> {
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

    pub(crate) async fn live_mcp_tools(
        &self,
        server_name: &str,
    ) -> Option<Vec<github_copilot_sdk::rpc::McpTools>> {
        let session = {
            let guard = self.chat.lock().await;
            let dated = guard.as_ref()?;
            dated.worker.session()
        };
        let request = github_copilot_sdk::rpc::McpListToolsRequest {
            server_name: server_name.to_owned(),
        };
        match session.rpc().mcp().list_tools(request).await {
            Ok(result) => Some(result.tools),
            Err(e) => {
                tracing::warn!(server = server_name, err = %e, "registry.live_mcp_tools.rpc_failed");
                Some(Vec::new())
            }
        }
    }

    pub(crate) async fn goals(&self) -> Result<Arc<BatchWorker>, AppError> {
        let _lifecycle = self.lifecycle.read().await;
        self.get_or_create_worker(&self.goals, WorkerClass::Goals, |s| {
            BatchWorker::new(WorkerClass::Goals, s, Arc::clone(&self.lifecycle))
        })
        .await
    }

    pub(crate) async fn consolidate(&self) -> Result<Arc<BatchWorker>, AppError> {
        let _lifecycle = self.lifecycle.read().await;
        self.get_or_create_worker(&self.consolidate, WorkerClass::Consolidate, |s| {
            BatchWorker::new(WorkerClass::Consolidate, s, Arc::clone(&self.lifecycle))
        })
        .await
    }

    pub(crate) async fn vision(&self) -> Result<Arc<VisionWorker>, AppError> {
        let _lifecycle = self.lifecycle.read().await;
        self.get_or_create_worker(&self.vision, WorkerClass::Vision, |s| {
            VisionWorker::new(s, Arc::clone(&self.lifecycle))
        })
        .await
    }

    /// Lazy-init pattern shared by goals/consolidate/vision. Locks
    /// the slot, returns the cached `Arc<W>` if present, otherwise calls
    /// `create_or_resume` for the right class and constructs via `build`.
    /// Chat is special-cased (date-rotated cache) so it has its own method.
    async fn get_or_create_worker<W, F>(
        &self,
        slot: &Mutex<Option<Arc<W>>>,
        class: WorkerClass,
        build: F,
    ) -> Result<Arc<W>, AppError>
    where
        F: FnOnce(Arc<Session>) -> Arc<W>,
    {
        let mut guard = slot.lock().await;
        if let Some(w) = guard.as_ref() {
            return Ok(Arc::clone(w));
        }
        let session = self.create_or_resume(class).await?;
        let worker = build(session);
        *guard = Some(Arc::clone(&worker));
        Ok(worker)
    }

    pub(crate) async fn chat(&self) -> Result<Arc<CopilotChatWorker>, AppError> {
        let _lifecycle = self.lifecycle.read().await;
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
        let worker = CopilotChatWorker::new(session, chat_model, Arc::clone(&self.lifecycle));
        *guard = Some(DatedChatWorker {
            date: today,
            worker: Arc::clone(&worker),
        });
        Ok(worker)
    }

    /// Tear down every worker + the SDK client. Used by both `reload()`
    /// (engine config changed — caller follows up with `forget_session_ids`)
    /// and `shutdown_all()` (graceful daemon stop — preserves session IDs
    /// on disk so next boot can resume).
    async fn shutdown_workers(&self) {
        if let Some(w) = self.goals.lock().await.take() {
            log_shutdown("goals", w.shutdown().await);
        }
        if let Some(w) = self.consolidate.lock().await.take() {
            log_shutdown("consolidate", w.shutdown().await);
        }
        if let Some(w) = self.vision.lock().await.take() {
            log_shutdown("vision", w.shutdown().await);
        }
        if let Some(dated) = self.chat.lock().await.take() {
            log_shutdown("chat", dated.worker.shutdown().await);
        }
        self.client.stop().await;
    }

    async fn forget_session_ids(&self) {
        let now = Local::now();
        for class in [
            WorkerClass::Goals,
            WorkerClass::Consolidate,
            WorkerClass::Vision,
            WorkerClass::Chat,
        ] {
            if let Err(e) = self.session_store.forget(class, now).await {
                tracing::warn!(class = %class.name(), err = %e, "registry.forget_session_id_failed");
            }
        }
    }

    /// Engine-config reload: tear down workers AND forget session IDs.
    /// `ResumeSessionConfig` cannot override `model` so on next access the
    /// workers must fresh-create against the new engine config.
    pub(crate) async fn reload(&self) {
        let _lifecycle = self.lifecycle.write().await;
        self.shutdown_workers().await;
        self.forget_session_ids().await;
        tracing::info!("registry.reload_complete");
    }

    pub(crate) async fn apply_mcp_config(
        &self,
        servers: IndexMap<String, McpServerConfig>,
        excluded_tools: Vec<String>,
    ) {
        let _lifecycle = self.lifecycle.write().await;
        *self.mcp.write().await = McpRuntimeConfig {
            servers,
            excluded_tools,
        };
        self.shutdown_workers().await;
        tracing::info!("registry.mcp_reload_complete");
    }

    /// Model-only reload. SDK 1.0 can override the model on resume, so retain
    /// session IDs and conversation history while rebuilding workers.
    pub(crate) async fn reload_soft(&self) {
        let _lifecycle = self.lifecycle.write().await;
        self.shutdown_workers().await;
        tracing::info!("registry.reload_soft_complete");
    }

    /// Privacy purge: stop every live worker and durably remove every local
    /// resumable session ID, including historical daily chat IDs.
    pub(crate) async fn purge_sessions(&self) -> Result<(), AppError> {
        let _lifecycle = self.lifecycle.write().await;
        if let Some(worker) = self.goals.lock().await.take() {
            worker
                .destroy()
                .await
                .map_err(|error| AppError::Internal(error.to_string()))?;
        }
        if let Some(worker) = self.consolidate.lock().await.take() {
            worker
                .destroy()
                .await
                .map_err(|error| AppError::Internal(error.to_string()))?;
        }
        if let Some(worker) = self.vision.lock().await.take() {
            worker
                .destroy()
                .await
                .map_err(|error| AppError::Internal(error.to_string()))?;
        }
        if let Some(dated) = self.chat.lock().await.take() {
            dated
                .worker
                .destroy()
                .await
                .map_err(|error| AppError::Internal(error.to_string()))?;
        }
        self.client.stop().await;
        crate::util::durable_fs::durable_remove_dir_all(&self.data_dir.join("workers")).await?;
        Ok(())
    }

    /// Graceful daemon stop. Tears down workers + client but PRESERVES the
    /// on-disk session IDs so the next boot's `create_or_resume` finds them
    /// and resumes the chat thread. Without this distinction (i.e. before:
    /// `shutdown_all() = reload()`) every clean `ctrl_c` wiped chat history
    /// and only SIGKILL preserved it.
    pub(crate) async fn shutdown_all(&self) {
        let _lifecycle = self.lifecycle.write().await;
        self.shutdown_workers().await;
        tracing::info!("registry.shutdown_complete");
    }

    async fn create_or_resume(&self, class: WorkerClass) -> Result<Arc<Session>, AppError> {
        let client = self.client.ensure_started().await?;
        let now = Local::now();
        let mcp = self.mcp.read().await;
        let handler = BobeHandler::new(class, &mcp.excluded_tools);
        let hooks = BobeHooks::new(class, Arc::clone(&self.memory_file), self.voice.clone());

        let engine_snapshot = self.config.load().engine.clone();
        let (class_model, class_provider) = session_extras_for_class(&engine_snapshot, class);
        let mut tools = ToolSet::new()
            .add_builtin_many(BUILTIN_TOOLS_ISOLATED)
            .map_err(|e| AppError::Config(format!("Copilot built-in tool policy: {e}")))?;
        if class == WorkerClass::Chat && !mcp.servers.is_empty() {
            tools = tools
                .add_mcp("*")
                .map_err(|e| AppError::Config(format!("Copilot MCP tool policy: {e}")))?;
        }
        let available_tools = tools.into_vec();
        let large_output_directory = self.data_dir.join("tool-output");
        tokio::fs::create_dir_all(&large_output_directory).await?;
        let large_output = LargeToolOutputConfig::new()
            .with_enabled(true)
            .with_max_size_bytes(64 * 1024)
            .with_output_directory(large_output_directory);

        // Session mode set at runtime via `apply_runtime_mode`, not via `SessionConfig`.
        let cfg_template = || {
            let mut cfg = SessionConfig::default()
                .with_client_name("BoBe")
                .with_permission_handler(Arc::clone(&handler) as _)
                .with_hooks(Arc::clone(&hooks) as _)
                .with_streaming(class == WorkerClass::Chat)
                .with_infinite_sessions(InfiniteSessionConfig::new())
                .with_working_directory(self.data_dir.clone())
                .with_enable_config_discovery(false)
                .with_enable_file_hooks(false)
                .with_enable_host_git_operations(false)
                .with_enable_skills(true)
                .with_enable_session_telemetry(false)
                .with_memory(MemoryConfiguration::disabled())
                .with_large_output(large_output.clone())
                .with_available_tools(available_tools.clone())
                .with_excluded_tools(mcp.excluded_tools.clone());
            if let Some(skill_dir) = self.skill_dir(class) {
                cfg = cfg.with_skill_directories([skill_dir]);
            }
            if let Some(max_ai_credits) = class.max_ai_credits() {
                cfg = cfg.with_session_limits(SessionLimitsConfig {
                    max_ai_credits: Some(max_ai_credits),
                });
            }
            if class == WorkerClass::Chat && !mcp.servers.is_empty() {
                cfg = cfg.with_mcp_servers(mcp.servers.clone());
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
                .with_client_name("BoBe")
                .with_permission_handler(Arc::clone(&handler) as _)
                .with_hooks(Arc::clone(&hooks) as _)
                .with_streaming(class == WorkerClass::Chat)
                .with_infinite_sessions(InfiniteSessionConfig::new())
                .with_working_directory(self.data_dir.clone())
                .with_enable_config_discovery(false)
                .with_enable_file_hooks(false)
                .with_enable_host_git_operations(false)
                .with_enable_skills(true)
                .with_enable_session_telemetry(false)
                .with_memory(MemoryConfiguration::disabled())
                .with_large_output(large_output.clone())
                .with_available_tools(available_tools.clone())
                .with_excluded_tools(mcp.excluded_tools.clone());
            if let Some(skill_dir) = self.skill_dir(class) {
                resume_cfg = resume_cfg.with_skill_directories([skill_dir]);
            }
            if let Some(max_ai_credits) = class.max_ai_credits() {
                resume_cfg = resume_cfg.with_session_limits(SessionLimitsConfig {
                    max_ai_credits: Some(max_ai_credits),
                });
            }
            if class == WorkerClass::Chat && !mcp.servers.is_empty() {
                resume_cfg = resume_cfg.with_mcp_servers(mcp.servers.clone());
            }
            if let Some(model) = class_model.clone() {
                resume_cfg = resume_cfg.with_model(model);
            }
            if let Some(p) = class_provider.clone() {
                resume_cfg = resume_cfg.with_provider(p);
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
