//! Streaming text → sentence pipeline. `MarkdownStripper` + `SentenceBuffer`
//! chained: removes markdown, emits whole sentences on confirmed
//! terminator, pushes to the Kokoro `mpsc<String>`. Full channel → warn
//! + drop (sentences are independent; don't back-pressure the LLM).

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
