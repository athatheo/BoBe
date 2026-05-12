//! Single-slot voice sink shared by AppState. The voice WS handler stashes
//! its outbound sender here on connect; BobeHooks reads it from the SDK
//! hook context to push cached PCM (per-tool fillers, error recovery)
//! directly to the active client.
//!
//! Single-slot is correct: `UserMessageGuard` single-flights voice turns,
//! so only one client receives a filler at a time. If multi-client voice
//! coexistence ever lands, this expands to a session-keyed registry
//! without changing hook callers.

use std::sync::Arc;

use axum::extract::ws::Message;
use tokio::sync::{RwLock, mpsc};
use tracing::warn;

use crate::speech::protocol::{FLAG_FILLER, encode_tts_frame};
use crate::voice::filler_library::{FillerKind, FillerLibrary};
use crate::voice::opus::{encode_pcm_with, make_opus_encoder};

pub(crate) struct VoiceSink {
    inner: RwLock<Option<mpsc::Sender<Message>>>,
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
        }
    }

    /// Stash a sender for the lifetime of the returned guard. Drop clears
    /// the slot — so a WS disconnect mid-turn lets subsequent hook fires
    /// safely no-op rather than write to a closed channel.
    pub(crate) async fn install(self: &Arc<Self>, sink: mpsc::Sender<Message>) -> SinkGuard {
        *self.inner.write().await = Some(sink);
        SinkGuard {
            slot: Arc::clone(self),
        }
    }

    pub(crate) async fn get(&self) -> Option<mpsc::Sender<Message>> {
        self.inner.read().await.clone()
    }
}

/// RAII guard that clears the voice sink slot on drop. The clear runs in a
/// detached task so destruction stays sync from the caller's perspective.
pub(crate) struct SinkGuard {
    slot: Arc<VoiceSink>,
}

impl Drop for SinkGuard {
    fn drop(&mut self) {
        let slot = Arc::clone(&self.slot);
        tokio::spawn(async move {
            *slot.inner.write().await = None;
        });
    }
}

/// Encode a cached PCM filler as 20ms Opus frames and push them over the
/// active voice sink with `FLAG_FILLER`. No-op when no client is connected,
/// no library is loaded, or the kind isn't in the library.
pub(crate) async fn emit_filler(sink: &VoiceSink, library: &FillerLibrary, kind: FillerKind) {
    let Some(tx) = sink.get().await else {
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
        if tx.send(Message::Binary(framed.into())).await.is_err() {
            warn!("voice.sink_emit_send_failed");
            return;
        }
    }
}

/// Map a tool name from `PreToolUseInput.tool_name` to a filler intent.
/// Unknown tools fall through to `ToolGeneric`; the routing here is
/// case-insensitive and matches the canonical Copilot CLI names plus
/// common MCP aliases.
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
}
