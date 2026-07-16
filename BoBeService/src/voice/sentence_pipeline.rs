//! Streaming text → sentence pipeline. `MarkdownStripper` + `SentenceBuffer`
//! chained: removes markdown, emits whole sentences on confirmed
//! terminator, and returns whole sentences to the bounded TTS transport.

use std::time::Instant;

use crate::voice::telemetry::HIST_FIRST_SENTENCE_MS;
use crate::voice::text_prep::markdown_strip::MarkdownStripper;
use crate::voice::text_prep::sentence_buffer::SentenceBuffer;

pub(crate) struct SentencePipeline {
    md: MarkdownStripper,
    sb: SentenceBuffer,
    turn_start: Instant,
    first_sentence_recorded: bool,
}

impl SentencePipeline {
    pub(crate) fn new(turn_start: Instant) -> Self {
        Self {
            md: MarkdownStripper::new(),
            sb: SentenceBuffer::new(),
            turn_start,
            first_sentence_recorded: false,
        }
    }

    pub(crate) fn feed(&mut self, delta: &str) -> Vec<String> {
        let clean = self.md.feed(delta);
        if clean.is_empty() {
            return Vec::new();
        }
        let sentences = self.sb.feed(&clean);
        if !sentences.is_empty() {
            self.record_first_sentence();
        }
        sentences
    }

    pub(crate) fn flush(&mut self) -> Vec<String> {
        let sentences = self.sb.flush();
        if !sentences.is_empty() {
            self.record_first_sentence();
        }
        sentences
    }

    fn record_first_sentence(&mut self) {
        if self.first_sentence_recorded {
            return;
        }
        self.first_sentence_recorded = true;
        metrics::histogram!(HIST_FIRST_SENTENCE_MS)
            .record(self.turn_start.elapsed().as_secs_f64() * 1_000.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_all_sentences_without_transport_side_effects() {
        let mut pipeline = SentencePipeline::new(Instant::now());
        assert_eq!(
            pipeline.feed("**Hello.** Next sentence!"),
            vec!["Hello.", "Next sentence!"]
        );
        assert!(pipeline.flush().is_empty());
    }

    #[test]
    fn flush_returns_unterminated_tail() {
        let mut pipeline = SentencePipeline::new(Instant::now());
        assert!(pipeline.feed("Still speaking").is_empty());
        assert_eq!(pipeline.flush(), vec!["Still speaking"]);
    }
}
