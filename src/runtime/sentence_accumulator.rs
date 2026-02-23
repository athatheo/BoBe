//! Sentence accumulator — buffers streaming tokens into complete sentences.
//!
//! Splits streaming LLM text output at sentence boundaries so each sentence
//! can be dispatched individually. Purely synchronous, no async.

use regex::Regex;
use std::sync::LazyLock;

/// Abbreviations that end with a period but are NOT sentence boundaries.
#[allow(dead_code)]
const ABBREVIATIONS: &[&str] = &[
    "mr", "mrs", "ms", "dr", "prof", "sr", "jr", "st", "ave", "vs", "etc", "inc", "ltd", "corp",
    "dept", "univ", "gen", "gov", "sgt", "cpl", "pvt", "capt", "col", "maj", "lt", "cmdr", "adm",
    "rev", "hon", "pres", "approx", "est", "min", "max", "misc", "vol", "fig", "eq", "no", "op",
    "pt", "i.e", "e.g",
];

/// Pattern: sentence-ending punctuation followed by whitespace.
#[allow(dead_code)]
static SENTENCE_END_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([.!?])(\s+)").unwrap());

#[allow(dead_code)]
pub struct SentenceAccumulator {
    buffer: String,
    pending: Vec<String>,
    min_sentence_length: usize,
}

#[allow(dead_code)]
impl SentenceAccumulator {
    pub fn new(min_sentence_length: usize) -> Self {
        Self {
            buffer: String::new(),
            pending: Vec::new(),
            min_sentence_length,
        }
    }

    /// Current incomplete text that has not yet formed a sentence.
    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    /// Append a text delta and extract any complete sentences.
    pub fn feed(&mut self, delta: &str) {
        if delta.is_empty() {
            return;
        }
        self.buffer.push_str(delta);
        self.extract_sentences();
    }

    /// Return all complete sentences accumulated so far and clear them.
    pub fn drain_sentences(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending)
    }

    /// Return any remaining buffered text and reset the accumulator.
    pub fn flush(&mut self) -> String {
        let remainder = self.buffer.trim().to_owned();
        self.buffer.clear();
        self.pending.clear();
        remainder
    }

    /// Discard all state (for barge-in interruption).
    pub fn cancel(&mut self) {
        self.buffer.clear();
        self.pending.clear();
    }

    fn extract_sentences(&mut self) {
        let mut search_from = 0;

        loop {
            let haystack = &self.buffer[search_from..];
            let m = match SENTENCE_END_RE.find(haystack) {
                Some(m) => m,
                None => break,
            };

            let abs_start = search_from + m.start();
            let abs_end_punct = abs_start + 1; // include punctuation
            let abs_match_end = search_from + m.end();

            let candidate = self.buffer[..abs_end_punct].trim().to_owned();

            // Check abbreviation
            let punct_char = self.buffer.as_bytes().get(abs_start).copied().unwrap_or(0);
            if punct_char == b'.' && Self::is_abbreviation(&candidate) {
                search_from = abs_match_end;
                continue;
            }

            // Min length filter
            if self.min_sentence_length > 0 && candidate.len() < self.min_sentence_length {
                self.buffer = self.buffer[abs_end_punct..].trim_start().to_owned();
                search_from = 0;
                continue;
            }

            // Valid sentence
            self.pending.push(candidate);
            self.buffer = self.buffer[abs_match_end..].to_owned();
            search_from = 0;
        }
    }

    fn is_abbreviation(text: &str) -> bool {
        if !text.ends_with('.') {
            return false;
        }
        let without_period = &text[..text.len() - 1];
        let last_word = without_period
            .rsplit_once(char::is_whitespace)
            .map(|(_, w)| w)
            .unwrap_or(without_period);
        ABBREVIATIONS.contains(&last_word.to_lowercase().as_str())
    }
}

impl Default for SentenceAccumulator {
    fn default() -> Self {
        Self::new(0)
    }
}
