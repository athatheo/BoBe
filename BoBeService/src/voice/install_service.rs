//! Voice-model installer orchestrator. Mirrors `OllamaInstallService`:
//! single-flight Mutex, progress via `watch<VoiceInstallSnapshot>`,
//! cancel via side `watch<bool>`. Models download into `~/.bobe/models/`;
//! `on_complete` hot-swaps the AppState ArcSwap. Catalog in
//! `install_artifacts.rs`, extraction in `install_extract.rs`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use tokio::sync::{Mutex, watch};
use tokio::task::JoinHandle;
use tracing::{info, warn};

use crate::error::AppError;
use crate::voice::install_artifacts::{ARTIFACTS, ModelArtifact};
use crate::voice::install_extract::{extract_and_install, tempfile_dir};

// Re-export public types so external `crate::voice::install_service::Foo`
// paths keep working without forcing every caller to chase the file split.
pub(crate) use crate::voice::install_artifacts::{
    InstallStatus, ModelProgress, VoiceInstallSnapshot, VoiceModelKind,
};

/// Fired by `run()` on Ok-completion so bootstrap can re-run the engine
/// loader and ArcSwap the AppState snapshot — wizard hits "Continue" and
/// voice works without a daemon restart.
type OnCompleteCallback = Arc<dyn Fn() -> futures::future::BoxFuture<'static, ()> + Send + Sync>;

pub(crate) struct VoiceInstallService {
    http: reqwest::Client,
    models_root: PathBuf,
    state: Arc<Mutex<ServiceState>>,
    on_complete: OnCompleteCallback,
}

struct ServiceState {
    snapshot_tx: watch::Sender<VoiceInstallSnapshot>,
    snapshot_rx: watch::Receiver<VoiceInstallSnapshot>,
    in_flight: Option<JoinHandle<()>>,
    cancel_tx: Option<watch::Sender<bool>>,
}

impl VoiceInstallService {
    pub(crate) fn new(
        http: reqwest::Client,
        models_root: PathBuf,
        on_complete: OnCompleteCallback,
    ) -> Arc<Self> {
        let initial = VoiceInstallSnapshot {
            models: VoiceModelKind::all().map(ModelProgress::pending).collect(),
            status: InstallStatus::Idle,
            error: None,
        };
        let (snapshot_tx, snapshot_rx) = watch::channel(initial);
        Arc::new(Self {
            http,
            models_root,
            state: Arc::new(Mutex::new(ServiceState {
                snapshot_tx,
                snapshot_rx,
                in_flight: None,
                cancel_tx: None,
            })),
            on_complete,
        })
    }

    pub(crate) async fn subscribe(&self) -> watch::Receiver<VoiceInstallSnapshot> {
        self.state.lock().await.snapshot_rx.clone()
    }

    /// Per-kind on-disk presence check. Doesn't validate file contents —
    /// the engine load path catches truncated/corrupt models with a clear
    /// error.
    pub(crate) fn is_installed(&self, kind: VoiceModelKind) -> bool {
        let target = self.target_path(kind);
        if let Some(art) = ARTIFACTS.iter().find(|a| a.kind == kind) {
            if art.is_tarball {
                target.is_dir()
            } else {
                target.is_file()
            }
        } else {
            false
        }
    }

    pub(crate) fn target_path(&self, kind: VoiceModelKind) -> PathBuf {
        // SAFETY: ARTIFACTS is a compile-time constant array with one entry
        // per VoiceModelKind variant. Adding a variant without an entry
        // would only fail at this expect, not at compile time — we accept
        // that because the catalog and the enum live next to each other in
        // install_artifacts.rs and divergence is caught by the very next
        // `is_installed` call in tests.
        #[allow(clippy::expect_used)]
        let art = ARTIFACTS
            .iter()
            .find(|a| a.kind == kind)
            .expect("every VoiceModelKind has a manifest entry");
        self.models_root.join(art.target_subpath)
    }

    pub(crate) async fn start(self: &Arc<Self>) -> Result<(), AppError> {
        let mut state = self.state.lock().await;
        if let Some(h) = state.in_flight.as_ref()
            && !h.is_finished()
        {
            return Err(AppError::Conflict(
                "Voice install already in progress".into(),
            ));
        }
        let (cancel_tx, cancel_rx) = watch::channel(false);
        state.cancel_tx = Some(cancel_tx);
        let snapshot_tx = state.snapshot_tx.clone();
        snapshot_tx
            .send(VoiceInstallSnapshot {
                models: VoiceModelKind::all().map(ModelProgress::pending).collect(),
                status: InstallStatus::Running,
                error: None,
            })
            .ok();
        let svc = Arc::clone(self);
        let on_complete = Arc::clone(&self.on_complete);
        state.in_flight = Some(tokio::spawn(async move {
            let result = svc.run(snapshot_tx.clone(), cancel_rx).await;
            let (final_status, final_error) = match result {
                Ok(()) => (InstallStatus::Complete, None),
                Err(AppError::Canceled(_)) => (InstallStatus::Canceled, None),
                Err(e) => {
                    warn!(err = %e, "voice_install.failed");
                    (InstallStatus::Failed, Some(e.to_string()))
                }
            };
            // Hot-swap engines BEFORE flipping the snapshot status to
            // Complete — wizards polling /voice/install/status see
            // Complete only after the AppState ArcSwap has the new
            // engines, so the next /voice/stream connect actually works.
            if matches!(final_status, InstallStatus::Complete) {
                (on_complete)().await;
            }
            let snap = snapshot_tx.borrow().clone();
            snapshot_tx
                .send(VoiceInstallSnapshot {
                    status: final_status,
                    error: final_error,
                    ..snap
                })
                .ok();
        }));
        Ok(())
    }

    pub(crate) async fn cancel(&self) {
        let state = self.state.lock().await;
        if let Some(tx) = state.cancel_tx.as_ref() {
            tx.send(true).ok();
        }
    }

    async fn run(
        self: Arc<Self>,
        snapshot_tx: watch::Sender<VoiceInstallSnapshot>,
        cancel_rx: watch::Receiver<bool>,
    ) -> Result<(), AppError> {
        tokio::fs::create_dir_all(&self.models_root)
            .await
            .map_err(|e| AppError::Internal(format!("models_root mkdir: {e}")))?;

        for art in ARTIFACTS {
            if *cancel_rx.borrow() {
                return Err(AppError::Canceled("Voice install canceled".into()));
            }
            if self.is_installed(art.kind) {
                info!(
                    kind = %art.kind.label(),
                    "voice_install.already_installed"
                );
                self.update_model(
                    &snapshot_tx,
                    art.kind,
                    ModelProgress::complete(art.kind, "already installed"),
                );
                continue;
            }
            self.install_one(art, &snapshot_tx, &cancel_rx).await?;
        }
        Ok(())
    }

    async fn install_one(
        &self,
        art: &ModelArtifact,
        snapshot_tx: &watch::Sender<VoiceInstallSnapshot>,
        cancel_rx: &watch::Receiver<bool>,
    ) -> Result<(), AppError> {
        info!(kind = %art.kind.label(), url = art.url, "voice_install.start");
        let tmp_dir = tempfile_dir().map_err(|e| AppError::Internal(format!("tmpdir: {e}")))?;
        let archive_name = art.url.rsplit('/').next().unwrap_or("download.bin");
        let archive_path = tmp_dir.join(archive_name);

        self.download_to(art, &archive_path, snapshot_tx, cancel_rx)
            .await?;

        if *cancel_rx.borrow() {
            return Err(AppError::Canceled("Voice install canceled".into()));
        }

        let final_target = self.models_root.join(art.target_subpath);
        if art.is_tarball {
            extract_and_install(&archive_path, &tmp_dir, &final_target).await?;
        } else {
            if let Some(parent) = final_target.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| AppError::Internal(format!("mkdir {}: {e}", parent.display())))?;
            }
            tokio::fs::rename(&archive_path, &final_target)
                .await
                .map_err(|e| AppError::Internal(format!("rename to target: {e}")))?;
        }
        drop(tokio::fs::remove_dir_all(&tmp_dir).await);

        self.update_model(
            snapshot_tx,
            art.kind,
            ModelProgress::complete(art.kind, "installed"),
        );
        info!(kind = %art.kind.label(), "voice_install.installed");
        Ok(())
    }

    async fn download_to(
        &self,
        art: &ModelArtifact,
        dest: &Path,
        snapshot_tx: &watch::Sender<VoiceInstallSnapshot>,
        cancel_rx: &watch::Receiver<bool>,
    ) -> Result<(), AppError> {
        let response = self
            .http
            .get(art.url)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("http get: {e}")))?
            .error_for_status()
            .map_err(|e| AppError::Internal(format!("http status: {e}")))?;
        let total = response.content_length();
        let mut stream = response.bytes_stream();
        let mut file = tokio::fs::File::create(dest)
            .await
            .map_err(|e| AppError::Internal(format!("create tmp: {e}")))?;
        use tokio::io::AsyncWriteExt;
        let mut got: u64 = 0;
        while let Some(chunk) = stream.next().await {
            if *cancel_rx.borrow() {
                return Err(AppError::Canceled("Voice install canceled".into()));
            }
            let chunk = chunk.map_err(|e| AppError::Internal(format!("http chunk: {e}")))?;
            file.write_all(&chunk)
                .await
                .map_err(|e| AppError::Internal(format!("write tmp: {e}")))?;
            got = got.saturating_add(chunk.len() as u64);
            let percent = total.map(|t| ((got as f64 / t as f64) * 100.0).min(100.0) as u8);
            self.update_model(
                snapshot_tx,
                art.kind,
                ModelProgress {
                    kind: art.kind,
                    label: art.kind.label(),
                    status: "downloading".into(),
                    bytes_downloaded: got,
                    bytes_total: total,
                    percent,
                },
            );
        }
        file.flush()
            .await
            .map_err(|e| AppError::Internal(format!("flush tmp: {e}")))?;
        Ok(())
    }

    fn update_model(
        &self,
        snapshot_tx: &watch::Sender<VoiceInstallSnapshot>,
        kind: VoiceModelKind,
        progress: ModelProgress,
    ) {
        let mut snap = snapshot_tx.borrow().clone();
        if let Some(slot) = snap.models.iter_mut().find(|m| m.kind == kind) {
            *slot = progress;
        }
        snapshot_tx.send(snap).ok();
    }

    /// Wait for any in-flight install to finish — used by shutdown paths.
    /// Block until any in-flight install completes (or polls every 50ms
    /// if there isn't one). Used by the graceful shutdown path so the
    /// install task doesn't outlive the http client / file-system
    /// resources it depends on.
    pub(crate) async fn await_idle(&self) {
        loop {
            let handle = {
                let state = self.state.lock().await;
                state
                    .in_flight
                    .as_ref()
                    .and_then(|h| (!h.is_finished()).then_some(()))
            };
            if handle.is_none() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}
