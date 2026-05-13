//! Voice-model artifact catalog + install snapshot DTOs. The catalog is
//! intentionally hard-coded — upstream URL drift requires an explicit code
//! update so a mismatched binary surfaces in the commit log rather than a
//! config file edit. Lives separately from the install service so the
//! install_service module focuses purely on the run-loop orchestration.

use serde::{Deserialize, Serialize};

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
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::StreamingStt => "streaming-stt",
            Self::Tts => "tts",
            Self::Vad => "vad",
            Self::SmartTurn => "smart-turn",
        }
    }

    pub(super) fn all() -> &'static [VoiceModelKind] {
        &[Self::StreamingStt, Self::Tts, Self::Vad, Self::SmartTurn]
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

pub(super) const ARTIFACTS: &[ModelArtifact] = &[
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
