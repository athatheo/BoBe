//! Streaming sentence buffer for TTS. Port of LiveKit's `_basic_sent.split_sentences`.
//!
//! Pushes text deltas in, emits complete-sentence strings out (in order). A
//! sentence is "complete" once a subsequent sentence has been seen — i.e., we
//! only emit when there are ≥2 sentences in the buffer; the last one stays as
//! potentially-still-growing. `flush()` releases the tail at end of stream.
//!
//! Handles common English abbreviations (Dr., Mr., U.S.A., e.g., etc.) so a
//! decimal or initialism doesn't trigger a false split.

const MIN_CTX_LEN: usize = 10;
const MIN_SENT_LEN: usize = 20;
const ABBREVIATIONS: &[&str] = &[
    "mr.", "mrs.", "ms.", "dr.", "st.", "jr.", "sr.", "inc.", "ltd.", "co.", "ph.d.", "m.d.",
    "u.s.", "u.k.", "u.s.a.", "etc.", "vs.", "i.e.", "e.g.", "a.m.", "p.m.", "no.", "fig.",
];

pub(crate) struct SentenceBuffer {
    buffer: String,
}

impl SentenceBuffer {
    pub(crate) fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    /// Add text and try to extract sentences that have been confirmed-ended.
    /// Returns empty until at least 2 sentences are buffered.
    pub(crate) fn feed(&mut self, text: &str) -> Vec<String> {
        self.buffer.push_str(text);
        if self.buffer.len() < MIN_CTX_LEN {
            return Vec::new();
        }
        let sentences = split_sentences(&self.buffer);
        if sentences.len() < 2 {
            return Vec::new();
        }
        let last = sentences.last().cloned().unwrap_or_default();
        let mut out: Vec<String> = sentences.into_iter().collect();
        out.pop();
        let out: Vec<String> = out
            .into_iter()
            .filter(|s| !s.is_empty() && (s.len() >= MIN_SENT_LEN || ends_with_terminator(s)))
            .collect();
        self.buffer = last;
        out
    }

    /// Force-emit any remaining buffer content (end of LLM stream).
    pub(crate) fn flush(&mut self) -> Vec<String> {
        let remaining = std::mem::take(&mut self.buffer);
        let trimmed = remaining.trim();
        if trimmed.is_empty() {
            Vec::new()
        } else {
            vec![trimmed.to_string()]
        }
    }
}

fn ends_with_terminator(s: &str) -> bool {
    matches!(s.trim_end().chars().last(), Some('!' | '?' | '.'))
}

fn split_sentences(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut sentences = Vec::new();
    let mut start = 0_usize;
    let mut i = 0_usize;

    while i < chars.len() {
        let c = chars[i];
        if !matches!(c, '.' | '!' | '?') {
            i += 1;
            continue;
        }

        let next = chars.get(i + 1).copied();
        let next_next = chars.get(i + 2).copied();
        let is_terminator = match next {
            None => true,
            Some(n) if n.is_whitespace() => match next_next {
                None => true,
                Some(nn) => nn.is_uppercase() || nn == '"' || nn == '\'' || nn == '\n',
            },
            _ => false,
        };

        if !is_terminator {
            i += 1;
            continue;
        }

        // Abbreviation false-positive check: case-fold the segment-so-far + this
        // punctuation, see if it ends in a known token like "dr." or "u.s."
        let seg: String = chars[start..=i].iter().collect();
        let lower = seg.to_lowercase();
        if ABBREVIATIONS.iter().any(|a| lower.ends_with(a)) {
            i += 1;
            continue;
        }

        sentences.push(seg.trim().to_string());
        i += 1;
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        start = i;
    }

    if start < chars.len() {
        let remainder: String = chars[start..].iter().collect();
        let trimmed = remainder.trim();
        if !trimmed.is_empty() {
            sentences.push(trimmed.to_string());
        }
    }

    sentences
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_simple_sentences() {
        let s = split_sentences("Hello world. How are you? I am fine!");
        assert_eq!(s, vec!["Hello world.", "How are you?", "I am fine!"]);
    }

    #[test]
    fn preserves_abbreviations() {
        let s = split_sentences("Dr. Smith was here. He said hello.");
        assert_eq!(s, vec!["Dr. Smith was here.", "He said hello."]);
    }

    #[test]
    fn preserves_decimals() {
        // "3.14" — the '.' is followed by a digit, not whitespace + uppercase, so no split
        let s = split_sentences("Pi is 3.14 approximately. Cool.");
        assert_eq!(s, vec!["Pi is 3.14 approximately.", "Cool."]);
    }

    #[test]
    fn buffer_emits_only_after_second_sentence() {
        let mut b = SentenceBuffer::new();
        assert!(b.feed("Hello world.").is_empty());
        // Single sentence isn't emitted until a second appears
        assert!(b.feed(" Goodbye").is_empty());
        // Now a second sentence boundary materialises
        let out = b.feed(" world. And more.");
        assert_eq!(out, vec!["Hello world.", "Goodbye world."]);
    }

    #[test]
    fn flush_drains_remainder() {
        let mut b = SentenceBuffer::new();
        drop(b.feed("One sentence remaining no terminator"));
        let out = b.flush();
        assert_eq!(out, vec!["One sentence remaining no terminator"]);
    }
}
