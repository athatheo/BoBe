//! Orchestrates the wizard's local-mode install flow: download Ollama
//! runtime → start it → pull required models. Each stage publishes
//! progress on its own `watch::Sender` so the SSE endpoint can stream
//! all three concurrently to the UI.
//!
//! Concurrency: only one install runs at a time. Holding a `Mutex`
//! around the in-flight task handle lets `POST /local-runtime/install`
//! return 409 if a second start is requested mid-pull.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, watch};
use tokio::task::JoinHandle;
use tracing::{info, warn};

use crate::binary_manager::{BinaryManager, DownloadProgress};
use crate::error::AppError;
use crate::ollama_manager::{OllamaManager, PullProgress};

/// One stage of the install pipeline. UI binds a progress bar to each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InstallStage {
    Runtime,
    TextModel,
    BatchModel,
    VisionModel,
}

impl InstallStage {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            InstallStage::Runtime => "runtime",
            InstallStage::TextModel => "chat-model",
            InstallStage::BatchModel => "batch-model",
            InstallStage::VisionModel => "vision-model",
        }
    }
}

/// Snapshot of the install pipeline's state. Latest values for each
/// stage; the SSE endpoint emits these on each tick.
#[derive(Debug, Clone, Default)]
pub(crate) struct InstallSnapshot {
    pub(crate) runtime: DownloadProgress,
    pub(crate) chat_model: PullProgress,
    pub(crate) batch_model: PullProgress,
    pub(crate) vision_model: PullProgress,
    pub(crate) status: InstallStatus,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum InstallStatus {
    #[default]
    Idle,
    Running,
    Complete,
    Canceled,
    Failed(String),
}

/// Inputs the wizard sends to start an install.
#[derive(Debug, Clone)]
pub(crate) struct InstallRequest {
    pub(crate) chat_model: String,
    /// Same value as `chat_model` if the user wants one model for both;
    /// distinct value if they want a smaller batch model.
    pub(crate) batch_model: String,
    pub(crate) vision_model: String,
}

pub(crate) struct OllamaInstallService {
    binary_manager: Arc<BinaryManager>,
    ollama_manager: Arc<OllamaManager>,
    data_dir: PathBuf,
    state: Arc<Mutex<ServiceState>>,
}

struct ServiceState {
    snapshot_tx: watch::Sender<InstallSnapshot>,
    snapshot_rx: watch::Receiver<InstallSnapshot>,
    in_flight: Option<JoinHandle<()>>,
    cancel_tx: Option<watch::Sender<bool>>,
}

impl OllamaInstallService {
    pub(crate) fn new(
        binary_manager: Arc<BinaryManager>,
        ollama_manager: Arc<OllamaManager>,
        data_dir: PathBuf,
    ) -> Arc<Self> {
        let (snapshot_tx, snapshot_rx) = watch::channel(InstallSnapshot::default());
        Arc::new(Self {
            binary_manager,
            ollama_manager,
            data_dir,
            state: Arc::new(Mutex::new(ServiceState {
                snapshot_tx,
                snapshot_rx,
                in_flight: None,
                cancel_tx: None,
            })),
        })
    }

    /// SSE endpoint subscribes here to receive install snapshots.
    pub(crate) async fn subscribe(&self) -> watch::Receiver<InstallSnapshot> {
        self.state.lock().await.snapshot_rx.clone()
    }

    /// Start an install if none is in flight; otherwise returns
    /// `AppError::Conflict`.
    pub(crate) async fn start(self: &Arc<Self>, req: InstallRequest) -> Result<(), AppError> {
        let mut state = self.state.lock().await;
        if let Some(h) = state.in_flight.as_ref()
            && !h.is_finished()
        {
            return Err(AppError::Conflict("Install already in progress".into()));
        }

        let (cancel_tx, cancel_rx) = watch::channel(false);
        state.cancel_tx = Some(cancel_tx);

        let svc = Arc::clone(self);
        let snapshot_tx = state.snapshot_tx.clone();
        snapshot_tx
            .send(InstallSnapshot {
                status: InstallStatus::Running,
                ..InstallSnapshot::default()
            })
            .ok();
        state.in_flight = Some(tokio::spawn(async move {
            let result = svc.run(req, snapshot_tx.clone(), cancel_rx).await;
            let final_status = match result {
                Ok(()) => InstallStatus::Complete,
                Err(AppError::Conflict(msg)) if msg.contains("canceled") => {
                    InstallStatus::Canceled
                }
                Err(e) => {
                    warn!(err = %e, "ollama_install.failed");
                    InstallStatus::Failed(e.to_string())
                }
            };
            let snap = snapshot_tx.borrow().clone();
            snapshot_tx
                .send(InstallSnapshot {
                    status: final_status,
                    ..snap
                })
                .ok();
        }));

        Ok(())
    }

    /// Signal the in-flight install (if any) to bail out.
    pub(crate) async fn cancel(&self) {
        let state = self.state.lock().await;
        if let Some(tx) = state.cancel_tx.as_ref() {
            tx.send(true).ok();
        }
    }

    async fn run(
        self: Arc<Self>,
        req: InstallRequest,
        snapshot_tx: watch::Sender<InstallSnapshot>,
        cancel_rx: watch::Receiver<bool>,
    ) -> Result<(), AppError> {
        // Stage 1: ensure managed Ollama binary exists on disk.
        let (runtime_tx, mut runtime_rx) = watch::channel(DownloadProgress::default());
        let runtime_snapshot_tx = snapshot_tx.clone();
        let runtime_pump = tokio::spawn(async move {
            while runtime_rx.changed().await.is_ok() {
                let progress = runtime_rx.borrow().clone();
                let snap = runtime_snapshot_tx.borrow().clone();
                runtime_snapshot_tx
                    .send(InstallSnapshot {
                        runtime: progress,
                        ..snap
                    })
                    .ok();
            }
        });

        // If Ollama is already running on :11434, skip the binary download.
        let needs_managed_binary = !self.ollama_manager.health_check().await;
        let binary_path = if needs_managed_binary {
            info!("ollama_install.runtime_missing_downloading");
            self.binary_manager
                .ensure_managed_ollama(&runtime_tx)
                .await?
        } else {
            info!("ollama_install.runtime_present_skipping_download");
            runtime_tx
                .send(DownloadProgress {
                    current_bytes: 0,
                    total_bytes: None,
                    percent: Some(100),
                })
                .ok();
            self.binary_manager.managed_binary_path()
        };
        runtime_pump.abort();

        if *cancel_rx.borrow() {
            return Err(AppError::Conflict("Install canceled".into()));
        }

        // Validate before starting daemon — catches partial extracts.
        if needs_managed_binary {
            self.binary_manager
                .validate_ollama_binary(&binary_path)
                .await?;
        }

        // Stage 2: ensure the daemon is running. Reuses external Ollama
        // if user already has one; spawns ours otherwise.
        self.ollama_manager
            .ensure_daemon_running(Some(&binary_path), true)
            .await?;

        if *cancel_rx.borrow() {
            return Err(AppError::Conflict("Install canceled".into()));
        }

        // Stage 3+4+5: pull each model. Skip if already installed.
        // Run sequentially — Ollama already serializes pulls on the
        // server side, and concurrent pulls just thrash the network.
        let installed = self
            .ollama_manager
            .list_installed_models()
            .await
            .unwrap_or_default();

        let snapshot_for_chat = snapshot_tx.clone();
        let cancel_rx_for_chat = cancel_rx.clone();
        self.maybe_pull_model(
            &req.chat_model,
            &installed,
            InstallStage::TextModel,
            &snapshot_for_chat,
            cancel_rx_for_chat,
        )
        .await?;

        // Skip the batch pull if the user picked the same model as chat.
        if req.batch_model != req.chat_model {
            let installed_now = self
                .ollama_manager
                .list_installed_models()
                .await
                .unwrap_or_default();
            let snapshot_for_batch = snapshot_tx.clone();
            let cancel_rx_for_batch = cancel_rx.clone();
            self.maybe_pull_model(
                &req.batch_model,
                &installed_now,
                InstallStage::BatchModel,
                &snapshot_for_batch,
                cancel_rx_for_batch,
            )
            .await?;
        } else {
            let snap = snapshot_tx.borrow().clone();
            snapshot_tx
                .send(InstallSnapshot {
                    batch_model: PullProgress {
                        status: "skipped (same as chat)".into(),
                        completed_bytes: None,
                        total_bytes: None,
                        percent: Some(100),
                    },
                    ..snap
                })
                .ok();
        }

        let installed_final = self
            .ollama_manager
            .list_installed_models()
            .await
            .unwrap_or_default();
        let snapshot_for_vision = snapshot_tx.clone();
        let cancel_rx_for_vision = cancel_rx.clone();
        self.maybe_pull_model(
            &req.vision_model,
            &installed_final,
            InstallStage::VisionModel,
            &snapshot_for_vision,
            cancel_rx_for_vision,
        )
        .await?;

        Ok(())
    }

    async fn maybe_pull_model(
        &self,
        name: &str,
        installed: &[String],
        stage: InstallStage,
        snapshot_tx: &watch::Sender<InstallSnapshot>,
        cancel_rx: watch::Receiver<bool>,
    ) -> Result<(), AppError> {
        let already_installed = installed.iter().any(|m| m == name);
        if already_installed {
            info!(model = name, stage = stage.name(), "ollama_install.model_already_installed");
            self.write_stage(snapshot_tx, stage, PullProgress {
                status: "already installed".into(),
                completed_bytes: None,
                total_bytes: None,
                percent: Some(100),
            });
            return Ok(());
        }

        let (pull_tx, mut pull_rx) = watch::channel(PullProgress::default());
        let pump_snapshot_tx = snapshot_tx.clone();
        let pump_stage = stage;
        let pump = tokio::spawn(async move {
            while pull_rx.changed().await.is_ok() {
                let progress = pull_rx.borrow().clone();
                let snap = pump_snapshot_tx.borrow().clone();
                let updated = match pump_stage {
                    InstallStage::TextModel => InstallSnapshot {
                        chat_model: progress,
                        ..snap
                    },
                    InstallStage::BatchModel => InstallSnapshot {
                        batch_model: progress,
                        ..snap
                    },
                    InstallStage::VisionModel => InstallSnapshot {
                        vision_model: progress,
                        ..snap
                    },
                    InstallStage::Runtime => snap, // unreachable
                };
                pump_snapshot_tx.send(updated).ok();
            }
        });

        let cancel_check = move || *cancel_rx.borrow();
        let result = self
            .ollama_manager
            .pull_model(name, &pull_tx, cancel_check)
            .await;
        pump.abort();
        // Give the spawned pump a moment to drain final state.
        tokio::time::sleep(Duration::from_millis(20)).await;
        result
    }

    fn write_stage(
        &self,
        snapshot_tx: &watch::Sender<InstallSnapshot>,
        stage: InstallStage,
        progress: PullProgress,
    ) {
        let snap = snapshot_tx.borrow().clone();
        let updated = match stage {
            InstallStage::TextModel => InstallSnapshot {
                chat_model: progress,
                ..snap
            },
            InstallStage::BatchModel => InstallSnapshot {
                batch_model: progress,
                ..snap
            },
            InstallStage::VisionModel => InstallSnapshot {
                vision_model: progress,
                ..snap
            },
            InstallStage::Runtime => snap, // not used here
        };
        snapshot_tx.send(updated).ok();
    }

    /// Where the managed Ollama binary lives — exposed for handlers
    /// that need to know whether to display "managed by BoBe" vs
    /// "your existing install."
    pub(crate) fn managed_binary_path(&self) -> PathBuf {
        OllamaManager::default_managed_binary(&self.data_dir)
    }
}
