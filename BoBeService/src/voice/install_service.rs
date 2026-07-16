//! Voice-model installer orchestrator. Mirrors `OllamaInstallService`:
//! single-flight Mutex, progress via `watch<VoiceInstallSnapshot>`,
//! cancel via side `watch<bool>`. Models download into `~/.bobe/models/`;
//! `on_complete` hot-swaps the AppState ArcSwap. Catalog in
//! `install_artifacts.rs`, extraction in `install_extract.rs`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

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
    http: Arc<reqwest::Client>,
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

fn safe_model_target(models_root: &Path, target_subpath: &str) -> Result<PathBuf, AppError> {
    let relative = Path::new(target_subpath);
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(AppError::Config(format!(
            "Invalid voice model target path: {target_subpath}"
        )));
    }
    Ok(models_root.join(relative))
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    clippy::items_after_test_module,
    reason = "focused path-validation tests sit next to their private helper"
)]
mod tests {
    use super::safe_model_target;
    use std::path::{Path, PathBuf};

    #[test]
    fn model_target_stays_beneath_models_root() {
        assert_eq!(
            safe_model_target(Path::new("/models"), "kokoro/model.onnx")
                .expect("relative target is valid"),
            PathBuf::from("/models/kokoro/model.onnx")
        );
    }

    #[test]
    fn model_target_rejects_absolute_and_traversal_paths() {
        for target in [
            "/tmp/model.onnx",
            "../model.onnx",
            "kokoro/../model.onnx",
            "",
        ] {
            assert!(safe_model_target(Path::new("/models"), target).is_err());
        }
    }
}

impl VoiceInstallService {
    pub(crate) fn new(
        http: Arc<reqwest::Client>,
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

    #[expect(
        clippy::expect_used,
        reason = "compile-time artifact catalog is exhaustive and validated by tests"
    )]
    pub(crate) fn target_path(&self, kind: VoiceModelKind) -> PathBuf {
        let art = ARTIFACTS
            .iter()
            .find(|a| a.kind == kind)
            .expect("every VoiceModelKind has a manifest entry");
        safe_model_target(&self.models_root, art.target_subpath)
            .expect("compile-time voice artifact target must remain relative")
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
        drop(snapshot_tx.send_replace(VoiceInstallSnapshot {
            models: VoiceModelKind::all().map(ModelProgress::pending).collect(),
            status: InstallStatus::Running,
            error: None,
        }));
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
            drop(snapshot_tx.send_replace(VoiceInstallSnapshot {
                status: final_status,
                error: final_error,
                ..snap
            }));
        }));
        Ok(())
    }

    pub(crate) async fn cancel(&self) {
        let state = self.state.lock().await;
        if let Some(tx) = state.cancel_tx.as_ref() {
            tx.send_replace(true);
        }
    }

    async fn run(
        self: Arc<Self>,
        snapshot_tx: watch::Sender<VoiceInstallSnapshot>,
        cancel_rx: watch::Receiver<bool>,
    ) -> Result<(), AppError> {
        tokio::fs::create_dir_all(&self.models_root).await?;

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
        let tmp_dir = tempfile_dir()?;
        let result = self
            .install_one_in(art, snapshot_tx, cancel_rx, &tmp_dir)
            .await;
        if let Err(error) = tokio::fs::remove_dir_all(&tmp_dir).await
            && error.kind() != std::io::ErrorKind::NotFound
        {
            warn!(path = %tmp_dir.display(), error = %error, "voice_install.temp_cleanup_failed");
        }
        result
    }

    async fn install_one_in(
        &self,
        art: &ModelArtifact,
        snapshot_tx: &watch::Sender<VoiceInstallSnapshot>,
        cancel_rx: &watch::Receiver<bool>,
        tmp_dir: &Path,
    ) -> Result<(), AppError> {
        let archive_name = art.url.rsplit('/').next().unwrap_or("download.bin");
        let archive_path = tmp_dir.join(archive_name);

        self.download_to(art, &archive_path, snapshot_tx, cancel_rx)
            .await?;

        if *cancel_rx.borrow() {
            return Err(AppError::Canceled("Voice install canceled".into()));
        }

        let final_target = safe_model_target(&self.models_root, art.target_subpath)?;
        if art.is_tarball {
            extract_and_install(&archive_path, tmp_dir, &final_target).await?;
        } else {
            if let Some(parent) = final_target.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::rename(&archive_path, &final_target).await?;
        }
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
        let mut cancel = cancel_rx.clone();
        let response = tokio::select! {
            changed = cancel.changed() => {
                if changed.is_err() || *cancel.borrow() {
                    return Err(AppError::Canceled("Voice install canceled".into()));
                }
                return Err(AppError::Internal("Voice install cancellation channel changed unexpectedly".into()));
            }
            response = self.http.get(art.url).send() => response?.error_for_status()?,
        };
        let total = response.content_length();
        if total.is_some_and(|size| size > art.max_bytes) {
            return Err(AppError::Config(format!(
                "{} download Content-Length exceeds {} byte limit",
                art.kind.label(),
                art.max_bytes
            )));
        }
        let mut stream = response.bytes_stream();
        let mut file = tokio::fs::File::create(dest).await?;
        let mut integrity = crate::util::download_integrity::DownloadIntegrity::new(
            art.kind.label(),
            art.sha256,
            art.max_bytes,
        );
        use tokio::io::AsyncWriteExt;
        // Throttle progress emission. A 340 MB download produces 5-20k
        // chunks; without rate limiting that's the same number of
        // VoiceInstallSnapshot clones and watch-channel sends, all forwarded
        // to the SSE writer as JSON. Throttle to "percent changed OR ≥250ms
        // since last emit" — no perceptible UX cost, ~100× fewer events.
        let throttle = Duration::from_millis(250);
        let mut last_emit = Instant::now();
        let mut last_percent: Option<u8> = None;
        let mut got: u64 = 0;
        let make_progress = |got: u64, percent: Option<u8>| ModelProgress {
            kind: art.kind,
            label: art.kind.label(),
            status: "downloading".into(),
            bytes_downloaded: got,
            bytes_total: total,
            percent,
        };
        loop {
            let next = tokio::select! {
                changed = cancel.changed() => {
                    if changed.is_err() || *cancel.borrow() {
                        return Err(AppError::Canceled("Voice install canceled".into()));
                    }
                    continue;
                }
                chunk = stream.next() => chunk,
            };
            let Some(chunk) = next else { break };
            let chunk = chunk?;
            integrity.update(&chunk)?;
            file.write_all(&chunk).await?;
            got = got.saturating_add(chunk.len() as u64);
            let percent = total.map(|t| ((got as f64 / t as f64) * 100.0).min(100.0) as u8);
            if percent != last_percent || last_emit.elapsed() >= throttle {
                self.update_model(snapshot_tx, art.kind, make_progress(got, percent));
                last_emit = Instant::now();
                last_percent = percent;
            }
        }
        // Force final emit so the UI sees the last bytes even when the last
        // chunk landed inside the throttle window.
        let final_percent = total.map(|t| ((got as f64 / t as f64) * 100.0).min(100.0) as u8);
        self.update_model(snapshot_tx, art.kind, make_progress(got, final_percent));
        file.flush().await?;
        let verified_bytes = integrity.finish()?;
        debug_assert_eq!(got, verified_bytes);
        Ok(())
    }

    fn update_model(
        &self,
        snapshot_tx: &watch::Sender<VoiceInstallSnapshot>,
        kind: VoiceModelKind,
        progress: ModelProgress,
    ) {
        snapshot_tx.send_modify(|snapshot| {
            if let Some(slot) = snapshot.models.iter_mut().find(|model| model.kind == kind) {
                *slot = progress;
            }
        });
    }

    /// Wait for any in-flight install to finish — used by shutdown paths.
    /// We take the handle out under the lock and `.await` it without
    /// holding the lock. Shutdown is single-flight so the "what if start()
    /// races" concern doesn't apply here in practice.
    pub(crate) async fn await_idle(&self) {
        let handle = {
            let mut state = self.state.lock().await;
            match state.in_flight.as_ref() {
                Some(h) if !h.is_finished() => state.in_flight.take(),
                _ => None,
            }
        };
        if let Some(h) = handle {
            drop(h.await);
        }
    }
}
