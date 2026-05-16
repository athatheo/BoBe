//! Voice-model artifact catalog + install snapshot DTOs. The catalog is
//! intentionally hard-coded — upstream URL drift requires an explicit code
//! update so a mismatched binary surfaces in the commit log rather than a
//! config file edit. Lives separately from the install service so the
//! install_service module focuses purely on the run-loop orchestration.

use serde::{Deserialize, Serialize};

/// Voice-model artifacts the daemon needs on disk. Mode B: only the
/// daemon-side TTS engine (Kokoro) — STT/VAD/smart-turn moved to the
/// Swift client (FluidAudio). Wire-serialized as snake_case for the
/// install snapshot surfaced by `/voice/install/status` + the wizard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum VoiceModelKind {
    /// Kokoro v1.0 multilingual TTS (~340MB, sherpa-onnx).
    Tts,
}

impl VoiceModelKind {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Tts => "tts",
        }
    }

    /// Iterate every kind exactly once, derived from `ARTIFACTS` so the
    /// enum and the catalog can never drift — adding a kind without an
    /// artifact entry yields fewer iterations, not a runtime panic.
    pub(super) fn all() -> impl Iterator<Item = VoiceModelKind> {
        ARTIFACTS.iter().map(|a| a.kind)
    }
}

/// Source artifact + on-disk target for one model.
pub(super) struct ModelArtifact {
    pub(super) kind: VoiceModelKind,
    pub(super) url: &'static str,
    /// True when the URL points at a `.tar.bz2` archive whose contents
    /// extract into a sibling dir of the same basename. False when the
    /// URL is a single ONNX file.
    pub(super) is_tarball: bool,
    /// Final on-disk path the daemon's loader inspects.
    pub(super) target_subpath: &'static str,
}

pub(super) const ARTIFACTS: &[ModelArtifact] = &[ModelArtifact {
    kind: VoiceModelKind::Tts,
    url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/kokoro-multi-lang-v1_0.tar.bz2",
    is_tarball: true,
    target_subpath: "kokoro-multi-lang-v1_0",
}];

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
    pub(super) fn pending(kind: VoiceModelKind) -> Self {
        Self {
            kind,
            label: kind.label(),
            status: "pending".into(),
            bytes_downloaded: 0,
            bytes_total: None,
            percent: None,
        }
    }

    pub(super) fn complete(kind: VoiceModelKind, note: &str) -> Self {
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
