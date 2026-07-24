//! Single active-turn voice sink. The admitted turn installs its endpoint
//! output; hooks can never target an idle connection or a different endpoint.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::sync::RwLock;
use tracing::warn;

use crate::speech::protocol::{FLAG_FILLER, encode_tts_frame};
use crate::voice::filler_library::{FillerKind, FillerLibrary};
use crate::voice::opus::{encode_pcm_with, make_opus_encoder};
use crate::voice::output::VoiceOutput;

pub(crate) struct VoiceSink {
    inner: RwLock<Option<SinkSlot>>,
    /// Stamped onto each install; SinkGuard::drop only clears when it matches.
    next_generation: AtomicU64,
}

struct SinkSlot {
    output: VoiceOutput,
    generation: u64,
}

impl Default for VoiceSink {
    fn default() -> Self {
        Self::new()
    }
}

impl VoiceSink {
    pub(crate) fn new() -> Self {
        Self {
            inner: RwLock::new(None),
            next_generation: AtomicU64::new(1),
        }
    }

    /// Drop clears the slot; mid-turn disconnects make later hook fires no-op.
    pub(crate) async fn install(self: &Arc<Self>, output: VoiceOutput) -> SinkGuard {
        let generation = self.next_generation.fetch_add(1, Ordering::AcqRel);
        *self.inner.write().await = Some(SinkSlot { output, generation });
        SinkGuard {
            slot: Arc::clone(self),
            generation,
        }
    }

    pub(crate) async fn get(&self) -> Option<VoiceOutput> {
        self.inner
            .read()
            .await
            .as_ref()
            .map(|slot| slot.output.clone())
    }
}

/// RAII clear on drop; clear runs in a detached task. Generation match
/// preserves newer installs. Prefer explicit `uninstall_if_current`.
pub(crate) struct SinkGuard {
    slot: Arc<VoiceSink>,
    generation: u64,
}

impl Drop for SinkGuard {
    fn drop(&mut self) {
        let slot = Arc::clone(&self.slot);
        let generation = self.generation;
        tokio::spawn(async move {
            let mut guard = slot.inner.write().await;
            if guard.as_ref().is_some_and(|s| s.generation == generation) {
                *guard = None;
            }
        });
    }
}

/// 20ms Opus frames with `FLAG_FILLER`. No-op if disconnected / library missing.
pub(crate) async fn emit_filler(sink: &VoiceSink, library: &FillerLibrary, kind: FillerKind) {
    let Some(output) = sink.get().await else {
        return;
    };
    let Some(pcm) = library.get(kind) else {
        return;
    };
    let sample_rate = library.sample_rate();
    let Some(mut encoder) = make_opus_encoder(sample_rate) else {
        return;
    };
    let mut chunk_id: u64 = 0;
    for packet in encode_pcm_with(&mut encoder, &pcm, sample_rate) {
        let framed = encode_tts_frame(chunk_id, FLAG_FILLER, &packet);
        chunk_id = chunk_id.saturating_add(1);
        if !output.audio(framed).await {
            warn!("voice.sink_emit_send_failed");
            return;
        }
    }
}

/// Case-insensitive; unknown tools fall through to `ToolGeneric`.
pub(crate) fn filler_for_tool(tool_name: &str) -> FillerKind {
    let lower = tool_name.to_ascii_lowercase();
    if (lower.contains("web") && lower.contains("search")) || lower == "websearch" {
        FillerKind::ToolWebSearch
    } else if lower == "read" || lower == "read_file" || lower.contains("readfile") {
        FillerKind::ToolReadFile
    } else {
        FillerKind::ToolGeneric
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filler_for_tool_routes_known_names() {
        assert_eq!(filler_for_tool("web_search"), FillerKind::ToolWebSearch);
        assert_eq!(filler_for_tool("WebSearch"), FillerKind::ToolWebSearch);
        assert_eq!(filler_for_tool("Read"), FillerKind::ToolReadFile);
        assert_eq!(filler_for_tool("read_file"), FillerKind::ToolReadFile);
        assert_eq!(filler_for_tool("bash"), FillerKind::ToolGeneric);
        assert_eq!(filler_for_tool("anything_else"), FillerKind::ToolGeneric);
    }

    #[tokio::test]
    async fn newer_install_survives_older_guard_drop() {
        let sink = Arc::new(VoiceSink::new());

        let (output_a, _rx_a) = VoiceOutput::channel(8);
        let guard_a = sink.install(output_a).await;

        let (output_b, _rx_b) = VoiceOutput::channel(8);
        let _guard_b = sink.install(output_b).await;

        // Simulate the older guard's late drop racing the newer install.
        drop(guard_a);
        // Yield so the spawned drop task runs.
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;

        // Newer install must still be live.
        assert!(
            sink.get().await.is_some(),
            "older guard drop must not clear newer slot"
        );
    }
}
