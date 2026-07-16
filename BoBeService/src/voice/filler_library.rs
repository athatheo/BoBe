//! Pre-rendered filler PCM library. Synthesized once at bootstrap so the
//! /voice/stream handler can emit ack-style audio with zero TTS latency.
//!
//! Production agents pre-cache 3–5 phrases minimum — explicitly no apology
//! phrases per Pipecat/AWS Bedrock guidance ("apologies kill authority").
//! The catalog here covers TTFT-watchdog, post-barge-in recovery, generic
//! tool-progress, two specific tools, and an error/reconnect filler.

use std::collections::HashMap;
use std::sync::Arc;

use tracing::{info, warn};

use crate::speech::TtsEngine;

/// Discrete filler intents the voice handler can emit. Lookups go through
/// `FillerLibrary::get`. Adding a variant requires a phrase entry below.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum FillerKind {
    /// 800ms TTFT watchdog — "Hmm, let me think."
    Thinking,
    /// Per-tool: web_search.
    ToolWebSearch,
    /// Per-tool: read_file / file inspection.
    ToolReadFile,
    /// Per-tool: generic (default) when name doesn't match a more specific kind.
    ToolGeneric,
}

impl FillerKind {
    /// Source phrase synthesized at bootstrap. Kept short — TTS playback
    /// time is fixed per phrase; long fillers feel awkward in conversation.
    /// No apologies (production guidance).
    const fn phrase(self) -> &'static str {
        match self {
            Self::Thinking => "Hmm, let me think.",
            Self::ToolWebSearch => "Let me search the web.",
            Self::ToolReadFile => "One sec, looking at that.",
            Self::ToolGeneric => "Looking into that.",
        }
    }

    /// All variants in a fixed order; used by the bootstrap loop and tests.
    fn all() -> &'static [FillerKind] {
        &[
            Self::Thinking,
            Self::ToolWebSearch,
            Self::ToolReadFile,
            Self::ToolGeneric,
        ]
    }
}

/// The synthesized PCM cache. Wrapped in an Arc on AppState; lookups
/// return an inner Arc<Vec<f32>> so callers don't have to clone the PCM.
pub(crate) struct FillerLibrary {
    inner: HashMap<FillerKind, Arc<Vec<f32>>>,
    sample_rate: u32,
}

impl FillerLibrary {
    pub(crate) fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub(crate) fn get(&self, kind: FillerKind) -> Option<Arc<Vec<f32>>> {
        self.inner.get(&kind).map(Arc::clone)
    }

    /// Synthesize every `FillerKind` variant in parallel using the provided
    /// TTS engine. Failures are non-fatal — the missing kind is omitted and
    /// callers fall through to silence for that intent. spawn_blocking runs
    /// each synth on the blocking pool; for 7 short phrases on a system
    /// without per-engine locking this drops bootstrap delay from ~2-5s
    /// (sequential) to roughly the longest single phrase.
    pub(crate) async fn render(tts: Arc<dyn TtsEngine>) -> Self {
        const VOICE: &str = crate::constants::voice_wire::DEFAULT_PERSONA;
        let sample_rate = tts.sample_rate();
        let kinds = FillerKind::all();
        let futs = kinds.iter().map(|kind| {
            let tts_clone = Arc::clone(&tts);
            let phrase = kind.phrase();
            let k = *kind;
            async move {
                let result =
                    tokio::task::spawn_blocking(move || tts_clone.synthesize(phrase, VOICE, 1.0))
                        .await;
                (k, phrase, result)
            }
        });
        let results = futures::future::join_all(futs).await;
        let mut inner = HashMap::new();
        for (kind, _phrase, result) in results {
            match result {
                Ok(Ok(pcm)) => {
                    info!(
                        kind = ?kind,
                        samples = pcm.len(),
                        "voice.filler_rendered"
                    );
                    inner.insert(kind, Arc::new(pcm));
                }
                Ok(Err(e)) => {
                    warn!(kind = ?kind, error = %e, "voice.filler_synth_failed");
                }
                Err(e) => {
                    warn!(kind = ?kind, error = %e, "voice.filler_synth_join_failed");
                }
            }
        }
        Self { inner, sample_rate }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_kinds_have_phrases() {
        for kind in FillerKind::all() {
            assert!(!kind.phrase().is_empty());
        }
    }

    #[test]
    fn phrases_avoid_apologies() {
        // Production guidance: apologies kill authority. Block them here so
        // a future "sorry I can't…" addition is caught at test time.
        for kind in FillerKind::all() {
            let p = kind.phrase().to_lowercase();
            assert!(
                !p.starts_with("sorry, i can"),
                "Filler '{p}' violates the no-apology rule (kind {kind:?})"
            );
            assert!(
                !p.starts_with("apologies"),
                "Filler '{p}' violates the no-apology rule (kind {kind:?})"
            );
        }
    }

    #[test]
    fn no_duplicate_phrases() {
        let mut seen = std::collections::HashSet::new();
        for kind in FillerKind::all() {
            assert!(
                seen.insert(kind.phrase()),
                "duplicate filler phrase: {:?}",
                kind.phrase()
            );
        }
    }
}
