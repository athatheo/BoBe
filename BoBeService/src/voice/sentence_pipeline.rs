//! Streaming text → sentence pipeline used by the voice WS turn task.
//!
//! Wraps two stateful streaming filters into one push interface:
//!   1. `MarkdownStripper` — removes markdown syntax from running LLM
//!      tokens so Kokoro doesn't speak literal asterisks.
//!   2. `SentenceBuffer` — emits whole sentences once their terminator is
//!      confirmed (handles abbreviations, decimals, etc.).
//!
//! Confirmed sentences are pushed onto an `mpsc::Sender<String>` that the
//! Kokoro task drains. A full channel surfaces as a warn log + drop —
//! sentences are independent and back-pressuring the LLM stream upstream
//! would only stall the whole turn for marginal benefit.

use tokio::sync::mpsc;
use tracing::warn;

use crate::voice::text_prep::markdown_strip::MarkdownStripper;
use crate::voice::text_prep::sentence_buffer::SentenceBuffer;

pub(crate) struct SentencePipeline {
    md: MarkdownStripper,
    sb: SentenceBuffer,
    tx: mpsc::Sender<String>,
}

impl SentencePipeline {
    pub(crate) fn new(tx: mpsc::Sender<String>) -> Self {
        Self {
            md: MarkdownStripper::new(),
            sb: SentenceBuffer::new(),
            tx,
        }
    }

    pub(crate) fn feed(&mut self, delta: &str) {
        let clean = self.md.feed(delta);
        if clean.is_empty() {
            return;
        }
        for sentence in self.sb.feed(&clean) {
            if self.tx.try_send(sentence).is_err() {
                warn!("voice.sentence_channel_full_drop");
            }
        }
    }

    pub(crate) fn flush(&mut self) {
        for sentence in self.sb.flush() {
            if self.tx.try_send(sentence).is_err() {
                warn!("voice.sentence_channel_full_flush_drop");
            }
        }
    }
}
