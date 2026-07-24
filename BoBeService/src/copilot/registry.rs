//! Chat rotates daily; other worker classes use stable per-class session IDs.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use arc_swap::ArcSwap;
use chrono::{Local, NaiveDate};
use futures::StreamExt;
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
use crate::services::goals::goals_service::GoalsService;

use super::client::ClientHandle;
use super::handler::BobeHandler;
use super::hooks::{BobeHooks, HooksVoiceContext};
use super::memory_file::MemoryFile;
use super::registry_session_factory::{
    apply_reasoning_effort, apply_runtime_mode, log_shutdown, session_extras_for_class,
};
use super::session_store::SessionStore;
use super::types::WorkerClass;
use super::workers::batch::BatchWorker;
use super::workers::chat::CopilotChatWorker;
use super::workers::vision::VisionWorker;

const WORKER_DISCONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
const WORKER_ABORT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);
const LIFECYCLE_DRAIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
const SESSION_DELETE_RPC_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
const SESSION_PURGE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(12);
const SESSION_DELETE_CONCURRENCY: usize = 8;

pub(crate) struct WorkerRegistry {
    client: Arc<ClientHandle>,
    config: Arc<ArcSwap<Config>>,
    session_store: SessionStore,
    memory_file: Arc<MemoryFile>,
    goals_service: Arc<GoalsService>,
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

#[derive(Clone)]
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
        goals_service: Arc<GoalsService>,
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
            goals_service,
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
        let (chat_model, _, chat_reasoning) =
            session_extras_for_class(&engine_snapshot, WorkerClass::Chat);
        let session = self.create_or_resume(WorkerClass::Chat).await?;
        let worker = CopilotChatWorker::new(
            session,
            chat_model,
            chat_reasoning,
            Arc::clone(&self.lifecycle),
        );
        *guard = Some(DatedChatWorker {
            date: today,
            worker: Arc::clone(&worker),
        });
        Ok(worker)
    }

    async fn disconnect_workers(&self) {
        if tokio::time::timeout(
            WORKER_DISCONNECT_TIMEOUT,
            self.disconnect_workers_unbounded(),
        )
        .await
        .is_err()
        {
            tracing::warn!(
                timeout_ms = WORKER_DISCONNECT_TIMEOUT.as_millis() as u64,
                "worker disconnect timed out; force-stopping Copilot client"
            );
            self.client.force_stop().await;
            self.clear_worker_slots().await;
        }
    }

    async fn abort_live_workers(&self) {
        if tokio::time::timeout(WORKER_ABORT_TIMEOUT, self.abort_live_workers_unbounded())
            .await
            .is_err()
        {
            tracing::warn!(
                timeout_ms = WORKER_ABORT_TIMEOUT.as_millis() as u64,
                "worker abort timed out; force-stopping Copilot client"
            );
            self.client.force_stop().await;
        }
    }

    async fn abort_live_workers_unbounded(&self) {
        let goals = self.goals.lock().await.clone();
        let consolidate = self.consolidate.lock().await.clone();
        let vision = self.vision.lock().await.clone();
        let chat = self
            .chat
            .lock()
            .await
            .as_ref()
            .map(|dated| Arc::clone(&dated.worker));

        if let Some(worker) = goals
            && let Err(error) = worker.abort().await
        {
            tracing::debug!(err = %error, "goals worker abort unavailable");
        }
        if let Some(worker) = consolidate
            && let Err(error) = worker.abort().await
        {
            tracing::debug!(err = %error, "consolidate worker abort unavailable");
        }
        if let Some(worker) = vision
            && let Err(error) = worker.abort().await
        {
            tracing::debug!(err = %error, "vision worker abort unavailable");
        }
        if let Some(worker) = chat
            && let Err(error) = worker.abort().await
        {
            tracing::debug!(err = %error, "chat worker abort unavailable");
        }
    }

    async fn begin_exclusive_lifecycle(
        &self,
    ) -> Result<tokio::sync::OwnedRwLockWriteGuard<()>, AppError> {
        // Poll the writer once before aborting so Tokio's fair RwLock queues it
        // ahead of any later turn readers. Otherwise a new turn can enter
        // during the abort window and be killed by the same reload.
        let writer = Arc::clone(&self.lifecycle).write_owned();
        tokio::pin!(writer);
        let immediate_guard = tokio::select! {
            biased;
            guard = &mut writer => Some(guard),
            () = std::future::ready(()) => None,
        };
        self.abort_live_workers().await;
        if let Some(guard) = immediate_guard {
            return Ok(guard);
        }
        if let Ok(guard) = tokio::time::timeout(LIFECYCLE_DRAIN_TIMEOUT, &mut writer).await {
            return Ok(guard);
        }

        tracing::warn!(
            timeout_ms = LIFECYCLE_DRAIN_TIMEOUT.as_millis() as u64,
            "worker lifecycle drain timed out; force-stopping Copilot client"
        );
        self.client.force_stop().await;
        let result = tokio::time::timeout(LIFECYCLE_DRAIN_TIMEOUT, &mut writer).await;
        if result.is_err() {
            self.clear_worker_slots().await;
        }
        result.map_err(|_| AppError::Internal("Copilot worker lifecycle did not drain".into()))
    }

    async fn disconnect_workers_unbounded(&self) {
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
    }

    async fn clear_worker_slots(&self) {
        self.goals.lock().await.take();
        self.consolidate.lock().await.take();
        self.vision.lock().await.take();
        self.chat.lock().await.take();
    }

    /// Tear down every worker + the SDK client. Used by both `reload()`
    /// (engine config changed — caller follows up with `forget_session_ids`)
    /// and `shutdown_all()` (graceful daemon stop — preserves session IDs
    /// on disk so next boot can resume).
    async fn shutdown_workers(&self) {
        self.disconnect_workers().await;
        self.client.stop().await;
    }

    /// Engine/provider reload: retire active IDs before creating fresh
    /// sessions. Failed SDK deletions remain discoverable by privacy purge.
    pub(crate) async fn reload(&self) {
        let Ok(_lifecycle) = self.begin_exclusive_lifecycle().await else {
            tracing::error!("registry.reload_lifecycle_failed");
            return;
        };
        if let Err(error) = self.session_store.retire_active().await {
            tracing::error!(%error, "registry.reload_retire_sessions_failed");
            return;
        }
        self.shutdown_workers().await;
        let session_ids = match self.session_store.load_all().await {
            Ok(ids) => ids,
            Err(error) => {
                tracing::error!(%error, "registry.reload_load_retired_sessions_failed");
                return;
            }
        };
        let deletion_result = self.delete_sessions_with_deadline(&session_ids).await;
        self.client.stop().await;
        match deletion_result {
            Ok(()) => {
                if let Err(error) = self.session_store.clear_retired().await {
                    tracing::warn!(%error, "registry.reload_clear_retired_failed");
                }
            }
            Err(error) => {
                tracing::warn!(%error, "registry.reload_sdk_delete_failed_ids_retained");
            }
        }
        tracing::info!("registry.reload_complete");
    }

    pub(crate) async fn apply_mcp_config(
        &self,
        servers: IndexMap<String, McpServerConfig>,
        excluded_tools: Vec<String>,
    ) {
        let Ok(_lifecycle) = self.begin_exclusive_lifecycle().await else {
            tracing::error!("registry.mcp_reload_lifecycle_failed");
            return;
        };
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
        let Ok(_lifecycle) = self.begin_exclusive_lifecycle().await else {
            tracing::error!("registry.reload_soft_lifecycle_failed");
            return;
        };
        self.shutdown_workers().await;
        tracing::info!("registry.reload_soft_complete");
    }

    /// Privacy purge: disconnect live workers, delete every persisted SDK
    /// session (including historical daily chat IDs), then remove BoBe's IDs.
    pub(crate) async fn purge_sessions(&self) -> Result<(), AppError> {
        let _lifecycle = self.begin_exclusive_lifecycle().await?;
        let session_ids = self.session_store.load_all().await?;
        self.disconnect_workers().await;

        let deletion_result = self.delete_sessions_with_deadline(&session_ids).await;
        self.client.stop().await;
        deletion_result?;

        self.session_store.clear_all().await?;
        Ok(())
    }

    async fn delete_sessions_with_deadline(
        &self,
        session_ids: &[github_copilot_sdk::SessionId],
    ) -> Result<(), AppError> {
        if let Ok(result) = tokio::time::timeout(
            SESSION_PURGE_TIMEOUT,
            self.delete_persisted_sessions(session_ids),
        )
        .await
        {
            result
        } else {
            self.client.force_stop().await;
            Err(AppError::Internal(
                "Copilot session deletion timed out; session IDs retained for retry".into(),
            ))
        }
    }

    async fn delete_persisted_sessions(
        &self,
        session_ids: &[github_copilot_sdk::SessionId],
    ) -> Result<(), AppError> {
        if session_ids.is_empty() {
            return Ok(());
        }

        let client = self.client.ensure_started().await?;
        let results = futures::stream::iter(session_ids.iter().cloned())
            .map(|session_id| Self::delete_persisted_session(Arc::clone(&client), session_id))
            .buffer_unordered(SESSION_DELETE_CONCURRENCY)
            .collect::<Vec<_>>()
            .await;
        let errors = results
            .into_iter()
            .filter_map(Result::err)
            .map(|error| error.to_string())
            .collect::<Vec<_>>();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(AppError::Internal(format!(
                "{} Copilot session deletion(s) failed; session IDs retained for retry: {}",
                errors.len(),
                errors.join("; ")
            )))
        }
    }

    async fn delete_persisted_session(
        client: Arc<github_copilot_sdk::Client>,
        session_id: github_copilot_sdk::SessionId,
    ) -> Result<(), AppError> {
        let Ok(metadata) = tokio::time::timeout(
            SESSION_DELETE_RPC_TIMEOUT,
            client.get_session_metadata(&session_id),
        )
        .await
        else {
            return Err(AppError::Internal(format!(
                "checking Copilot session {session_id} timed out"
            )));
        };
        let metadata = metadata.map_err(|error| {
            AppError::Internal(format!(
                "check Copilot session {session_id} before purge: {error}"
            ))
        })?;
        if metadata.is_none() {
            return Ok(());
        }

        let Ok(deletion) = tokio::time::timeout(
            SESSION_DELETE_RPC_TIMEOUT,
            client.delete_session(&session_id),
        )
        .await
        else {
            return Err(AppError::Internal(format!(
                "deleting Copilot session {session_id} timed out"
            )));
        };
        deletion.map_err(|error| {
            AppError::Internal(format!("delete Copilot session {session_id}: {error}"))
        })
    }

    /// Graceful daemon stop. Tears down workers + client but PRESERVES the
    /// on-disk session IDs so the next boot's `create_or_resume` finds them
    /// and resumes the chat thread. Without this distinction (i.e. before:
    /// `shutdown_all() = reload()`) every clean `ctrl_c` wiped chat history
    /// and only SIGKILL preserved it.
    pub(crate) async fn shutdown_all(&self) {
        let Ok(_lifecycle) = self.begin_exclusive_lifecycle().await else {
            tracing::error!("registry.shutdown_lifecycle_failed");
            self.clear_worker_slots().await;
            return;
        };
        self.shutdown_workers().await;
        tracing::info!("registry.shutdown_complete");
    }

    async fn create_or_resume(&self, class: WorkerClass) -> Result<Arc<Session>, AppError> {
        let client = self.client.ensure_started().await?;
        let now = Local::now();
        let mcp = self.mcp.read().await.clone();
        let handler = BobeHandler::new(class, &mcp.excluded_tools);
        let hooks = BobeHooks::new(class, Arc::clone(&self.memory_file), self.voice.clone());

        let engine_snapshot = self.config.load().engine.clone();
        let (class_model, class_provider, class_reasoning) =
            session_extras_for_class(&engine_snapshot, class);
        let mut tools = ToolSet::new()
            .add_builtin_many(BUILTIN_TOOLS_ISOLATED)
            .map_err(|e| AppError::Config(format!("Copilot built-in tool policy: {e}")))?;
        let domain_tools = if class == WorkerClass::Chat {
            for name in super::tools::CHAT_DOMAIN_TOOL_NAMES {
                tools = tools.add_custom(name).map_err(|e| {
                    AppError::Config(format!("Copilot custom tool policy for {name}: {e}"))
                })?;
            }
            super::tools::chat_domain_tools(
                Arc::clone(&self.memory_file),
                Arc::clone(&self.goals_service),
            )
        } else {
            Vec::new()
        };
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
                .with_tools(domain_tools.clone())
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
                .with_tools(domain_tools.clone())
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
                    // CLI may reassign session id on resume (per SDK docstring); re-save to avoid stale id.
                    let live_id = session.id().clone();
                    if live_id != saved {
                        tracing::info!(
                            class = %class.name(),
                            old_id = %saved,
                            new_id = %live_id,
                            "CLI reassigned session id on resume; updating store"
                        );
                        self.persist_session_id_or_delete(
                            class,
                            now,
                            &session,
                            Arc::clone(&client),
                            &live_id,
                        )
                        .await?;
                    }
                    apply_runtime_mode(&session, class).await?;
                    apply_reasoning_effort(
                        &session,
                        class,
                        class_model.as_deref(),
                        class_reasoning.as_deref(),
                    )
                    .await?;
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
                    self.session_store.retire(class, now, &saved).await?;
                }
            }
        }

        let session = client
            .create_session(cfg_template())
            .await
            .map_err(|e| AppError::Internal(format!("create_session {}: {e}", class.name())))?;
        let id = session.id().clone();
        self.persist_session_id_or_delete(class, now, &session, Arc::clone(&client), &id)
            .await?;
        apply_runtime_mode(&session, class).await?;
        apply_reasoning_effort(
            &session,
            class,
            class_model.as_deref(),
            class_reasoning.as_deref(),
        )
        .await?;
        tracing::info!(
            class = %class.name(),
            session_id = %id,
            "created new copilot session"
        );
        Ok(Arc::new(session))
    }

    async fn persist_session_id_or_delete(
        &self,
        class: WorkerClass,
        now: chrono::DateTime<Local>,
        session: &Session,
        client: Arc<github_copilot_sdk::Client>,
        session_id: &github_copilot_sdk::SessionId,
    ) -> Result<(), AppError> {
        if let Err(error) = self.session_store.save(class, now, session_id).await {
            if let Err(retire_error) = self.session_store.retire_untracked(session_id).await {
                tracing::error!(
                    class = %class.name(),
                    session_id = %session_id,
                    err = %retire_error,
                    "failed to ledger untracked Copilot session"
                );
            }
            if let Err(disconnect_error) = session.disconnect().await {
                tracing::warn!(
                    class = %class.name(),
                    err = %disconnect_error,
                    "session disconnect after ID persistence failure failed"
                );
            }
            if let Err(delete_error) =
                Self::delete_persisted_session(client, session_id.clone()).await
            {
                tracing::error!(
                    class = %class.name(),
                    session_id = %session_id,
                    err = %delete_error,
                    "session cleanup after ID persistence failure failed"
                );
            }
            return Err(error);
        }
        Ok(())
    }

    fn skill_dir(&self, class: WorkerClass) -> Option<PathBuf> {
        let p = self.data_dir.join("skills").join(class.name());
        if p.exists() { Some(p) } else { None }
    }
}
