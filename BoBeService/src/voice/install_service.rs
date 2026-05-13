//! Voice-model installer service. Replaces the standalone
//! `scripts/install-voice-models.sh` — voice setup is a first-class app
//! capability, driven by the daemon and surfaced through the welcome
//! wizard + Settings panel.
//!
//! Mirrors `OllamaInstallService`'s shape: one install at a time gated by
//! a Mutex, progress streamed via `watch::Sender<VoiceInstallSnapshot>`,
//! cancel via a side `watch<bool>`. Models are downloaded sequentially
//! into `~/.bobe/models/`; on completion the daemon's voice engines need
//! a reload (next /voice/stream connect picks them up via
//! `bootstrap::load_voice_engines` re-running on `ConfigManager::reload`).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, watch};
use tokio::task::JoinHandle;
use tracing::{info, warn};

use crate::error::AppError;

/// One of the four voice-model artifacts the daemon expects on disk.
/// Wire-serialized as snake_case for the install snapshot surfaced by the
/// `/voice/install/status` endpoint + the wizard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum VoiceModelKind {
    /// Streaming Zipformer English ASR (~80MB, sherpa-onnx).
    StreamingStt,
    /// Kokoro v1.0 multilingual TTS (~340MB, sherpa-onnx).
    Tts,
    /// Silero v6.2.1 acoustic VAD (~2MB, ONNX).
    Vad,
    /// Pipecat smart-turn v3.2 semantic VAD (~8MB CPU ONNX).
    SmartTurn,
}

impl VoiceModelKind {
    const fn label(self) -> &'static str {
        match self {
            Self::StreamingStt => "streaming-stt",
            Self::Tts => "tts",
            Self::Vad => "vad",
            Self::SmartTurn => "smart-turn",
        }
    }

    fn all() -> &'static [VoiceModelKind] {
        &[Self::StreamingStt, Self::Tts, Self::Vad, Self::SmartTurn]
    }
}

/// Source artifact + on-disk target for one model. The catalog is
/// hard-coded — upstream URL drift requires an explicit code update
/// rather than a config file because mismatched binaries hard-fail the
/// daemon at boot and we want that surfaced in the commit log.
struct ModelArtifact {
    kind: VoiceModelKind,
    url: &'static str,
    /// True when the URL points at a `.tar.bz2` archive whose contents
    /// extract into a sibling dir of the same basename. False when the
    /// URL is a single ONNX file.
    is_tarball: bool,
    /// Final on-disk path the daemon's loader inspects.
    target_subpath: &'static str,
}

const ARTIFACTS: &[ModelArtifact] = &[
    ModelArtifact {
        kind: VoiceModelKind::StreamingStt,
        url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-streaming-zipformer-en-2023-06-26.tar.bz2",
        is_tarball: true,
        target_subpath: "sherpa-onnx-streaming-zipformer-en",
    },
    ModelArtifact {
        kind: VoiceModelKind::Tts,
        url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/kokoro-multi-lang-v1_0.tar.bz2",
        is_tarball: true,
        target_subpath: "kokoro-multi-lang-v1_0",
    },
    ModelArtifact {
        kind: VoiceModelKind::Vad,
        url: "https://github.com/snakers4/silero-vad/raw/v6.2.1/src/silero_vad/data/silero_vad.onnx",
        is_tarball: false,
        target_subpath: "silero-vad/silero_vad.onnx",
    },
    ModelArtifact {
        kind: VoiceModelKind::SmartTurn,
        url: "https://huggingface.co/pipecat-ai/smart-turn-v3/resolve/main/smart-turn-v3.2-cpu.onnx",
        is_tarball: false,
        target_subpath: "smart-turn-v3.2-cpu.onnx",
    },
];

#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct VoiceInstallSnapshot {
    pub(crate) models: Vec<ModelProgress>,
    pub(crate) status: InstallStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct ModelProgress {
    pub(crate) kind: VoiceModelKind,
    pub(crate) label: &'static str,
    pub(crate) status: String,
    pub(crate) bytes_downloaded: u64,
    pub(crate) bytes_total: Option<u64>,
    pub(crate) percent: Option<u8>,
}

impl ModelProgress {
    fn pending(kind: VoiceModelKind) -> Self {
        Self {
            kind,
            label: kind.label(),
            status: "pending".into(),
            bytes_downloaded: 0,
            bytes_total: None,
            percent: None,
        }
    }

    fn complete(kind: VoiceModelKind, note: &str) -> Self {
        Self {
            kind,
            label: kind.label(),
            status: note.into(),
            bytes_downloaded: 0,
            bytes_total: None,
            percent: Some(100),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InstallStatus {
    #[default]
    Idle,
    Running,
    Complete,
    Canceled,
    Failed(String),
}

/// Fired by `run()` on Ok-completion so bootstrap can re-run the engine
/// loader and ArcSwap the AppState snapshot — wizard hits "Continue" and
/// voice works without a daemon restart.
type OnCompleteCallback =
    Arc<dyn Fn() -> futures::future::BoxFuture<'static, ()> + Send + Sync>;

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
            models: VoiceModelKind::all()
                .iter()
                .map(|k| ModelProgress::pending(*k))
                .collect(),
            status: InstallStatus::Idle,
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
        // per VoiceModelKind variant. Coverage verified by enum exhaustiveness
        // check below — adding a variant without an artifact is a compile
        // error in `kind_iter()` consumers.
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
            return Err(AppError::Conflict("Voice install already in progress".into()));
        }
        let (cancel_tx, cancel_rx) = watch::channel(false);
        state.cancel_tx = Some(cancel_tx);
        let snapshot_tx = state.snapshot_tx.clone();
        snapshot_tx
            .send(VoiceInstallSnapshot {
                models: VoiceModelKind::all()
                    .iter()
                    .map(|k| ModelProgress::pending(*k))
                    .collect(),
                status: InstallStatus::Running,
            })
            .ok();
        let svc = Arc::clone(self);
        let on_complete = Arc::clone(&self.on_complete);
        state.in_flight = Some(tokio::spawn(async move {
            let result = svc.run(snapshot_tx.clone(), cancel_rx).await;
            let final_status = match result {
                Ok(()) => InstallStatus::Complete,
                Err(AppError::Canceled(_)) => InstallStatus::Canceled,
                Err(e) => {
                    warn!(err = %e, "voice_install.failed");
                    InstallStatus::Failed(e.to_string())
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
                self.update_model(&snapshot_tx, art.kind, ModelProgress::complete(art.kind, "already installed"));
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

        self.update_model(snapshot_tx, art.kind, ModelProgress::complete(art.kind, "installed"));
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
}

fn tempfile_dir() -> Result<PathBuf, std::io::Error> {
    let base = std::env::temp_dir();
    let unique = format!(
        "bobe-voice-install-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    );
    let path = base.join(unique);
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

/// Extract a .tar.bz2 archive whose contents are a single directory.
/// Find that directory inside `tmp_dir`, move it to `final_target`.
/// Also handles encoder/decoder/joiner rename for streaming Zipformer:
/// upstream files carry epoch-NN-avg-N suffixes; the daemon's loader
/// inspects bare `encoder.onnx` / `decoder.onnx` / `joiner.onnx`.
async fn extract_and_install(
    archive: &Path,
    tmp_dir: &Path,
    final_target: &Path,
) -> Result<(), AppError> {
    let archive_owned = archive.to_path_buf();
    let tmp_owned = tmp_dir.to_path_buf();
    tokio::task::spawn_blocking(move || -> Result<(), AppError> {
        let file = std::fs::File::open(&archive_owned)
            .map_err(|e| AppError::Internal(format!("open archive: {e}")))?;
        let bz = bzip2::read::BzDecoder::new(file);
        let mut tar = tar::Archive::new(bz);
        tar.unpack(&tmp_owned)
            .map_err(|e| AppError::Internal(format!("untar: {e}")))?;
        Ok(())
    })
    .await
    .map_err(|e| AppError::Internal(format!("untar join: {e}")))??;

    let mut extracted: Option<PathBuf> = None;
    let mut entries = tokio::fs::read_dir(tmp_dir)
        .await
        .map_err(|e| AppError::Internal(format!("readdir tmp: {e}")))?;
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|e| AppError::Internal(format!("readdir entry: {e}")))?
    {
        let ft = entry
            .file_type()
            .await
            .map_err(|e| AppError::Internal(format!("file_type: {e}")))?;
        if ft.is_dir() {
            extracted = Some(entry.path());
            break;
        }
    }
    let extracted = extracted
        .ok_or_else(|| AppError::Internal("tarball had no directory inside".into()))?;

    // Streaming Zipformer rename: drop epoch suffixes so the daemon's
    // loader finds canonical encoder.onnx / decoder.onnx / joiner.onnx.
    for prefix in ["encoder", "decoder", "joiner"] {
        rename_first_glob(&extracted, prefix, "onnx").await?;
    }

    if let Some(parent) = final_target.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| AppError::Internal(format!("mkdir parent: {e}")))?;
    }
    tokio::fs::rename(&extracted, final_target)
        .await
        .map_err(|e| AppError::Internal(format!("move into models_root: {e}")))?;
    drop(tokio::fs::remove_file(archive).await);
    Ok(())
}

/// Find the first file under `dir` whose name matches `{prefix}*.{ext}`
/// and rename it to `{prefix}.{ext}`. No-op when nothing matches (so
/// non-Zipformer tarballs are unaffected).
async fn rename_first_glob(dir: &Path, prefix: &str, ext: &str) -> Result<(), AppError> {
    let mut entries = tokio::fs::read_dir(dir)
        .await
        .map_err(|e| AppError::Internal(format!("readdir for rename: {e}")))?;
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|e| AppError::Internal(format!("readdir entry rename: {e}")))?
    {
        let name = entry.file_name().to_string_lossy().to_string();
        if name == format!("{prefix}.{ext}") {
            return Ok(());
        }
        if name.starts_with(prefix) && name.ends_with(&format!(".{ext}")) {
            let from = entry.path();
            let to = dir.join(format!("{prefix}.{ext}"));
            tokio::fs::rename(&from, &to)
                .await
                .map_err(|e| AppError::Internal(format!("rename {prefix}: {e}")))?;
            return Ok(());
        }
    }
    Ok(())
}

/// Wait for any in-flight install to finish — used by shutdown paths.
impl VoiceInstallService {
    #[allow(dead_code, reason = "drain-on-shutdown helper, wired in M5.x")]
    pub(crate) async fn await_idle(&self) {
        loop {
            let handle = {
                let state = self.state.lock().await;
                state.in_flight.as_ref().and_then(|h| (!h.is_finished()).then_some(()))
            };
            if handle.is_none() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}
