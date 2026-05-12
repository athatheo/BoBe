//! Streaming sentence buffer for TTS. Port of LiveKit's `_basic_sent.split_sentences`.
//!
//! Pushes text deltas in, emits complete-sentence strings out (in order).
//! Semantics: a sentence is "complete" the moment we see a `.`, `!`, or `?`
//! followed by whitespace + an uppercase character (or end of buffer). Text
//! still being typed after the most recent terminator stays in the buffer
//! until either a subsequent terminator promotes it OR `flush()` is called
//! at end-of-stream.
//!
//! Handles common English abbreviations (`Dr.`, `Mr.`, `U.S.A.`, `e.g.`, `etc.`)
//! and bare decimals like `3.14` so they don't trigger false splits.

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

    /// Append `text` and return any sentences whose terminators are now
    /// confirmed. Text past the last terminator stays buffered until a
    /// subsequent feed promotes it (or `flush` drains everything).
    pub(crate) fn feed(&mut self, text: &str) -> Vec<String> {
        self.buffer.push_str(text);
        let (sentences, leftover) = split_at_terminators(&self.buffer);
        self.buffer = leftover;
        sentences
    }

    /// Drain whatever's buffered — call at end-of-stream so trailing text
    /// without a terminator (e.g. "Sure" with no trailing period) still
    /// gets synthesized.
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

/// Returns `(confirmed_sentences_in_order, leftover_after_last_terminator)`.
///
/// A `.`, `!`, or `?` confirms a sentence iff:
/// - it's the last character of `text`, OR
/// - it's followed by whitespace and the next non-whitespace character is
///   uppercase, `"`, `'`, or newline (matches LiveKit's default heuristic).
///
/// Abbreviations are rejected by case-folding the segment-so-far and looking
/// up its trailing token against `ABBREVIATIONS`.
fn split_at_terminators(text: &str) -> (Vec<String>, String) {
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

        // Abbreviation false-positive check: case-fold "<start..=i>", see
        // if it ends in a known token like "dr." or "u.s.a.".
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

    let leftover = if start < chars.len() {
        chars[start..].iter().collect::<String>().trim().to_string()
    } else {
        String::new()
    };
    (sentences, leftover)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_on_terminator_immediately() {
        let mut b = SentenceBuffer::new();
        let out = b.feed("Hello world.");
        assert_eq!(out, vec!["Hello world."]);
    }

    #[test]
    fn buffers_text_without_terminator() {
        let mut b = SentenceBuffer::new();
        assert!(b.feed("Hello world").is_empty());
        let out = b.feed(", and more.");
        assert_eq!(out, vec!["Hello world, and more."]);
    }

    #[test]
    fn splits_multiple_sentences_in_one_feed() {
        let mut b = SentenceBuffer::new();
        let out = b.feed("How are you? I am fine! Goodbye.");
        assert_eq!(out, vec!["How are you?", "I am fine!", "Goodbye."]);
    }

    #[test]
    fn preserves_abbreviation_dr() {
        let mut b = SentenceBuffer::new();
        // "Dr." followed by " S" looks like a sentence end, but the
        // abbreviation check stops the split.
        assert!(b.feed("Dr. Smith was here").is_empty());
        let out = b.feed(" today. He left.");
        assert_eq!(out, vec!["Dr. Smith was here today.", "He left."]);
    }

    #[test]
    fn preserves_decimal_numbers() {
        let mut b = SentenceBuffer::new();
        // "3.14" — period followed by digit, not whitespace+uppercase, so no split
        let out = b.feed("Pi is 3.14 approximately. Cool.");
        assert_eq!(out, vec!["Pi is 3.14 approximately.", "Cool."]);
    }

    #[test]
    fn preserves_initialism_us() {
        let mut b = SentenceBuffer::new();
        let out = b.feed("Travel to U.S. cities is fun. Try it.");
        assert_eq!(out, vec!["Travel to U.S. cities is fun.", "Try it."]);
    }

    #[test]
    fn flush_drains_unterminated_remainder() {
        let mut b = SentenceBuffer::new();
        drop(b.feed("One sentence remaining no terminator"));
        let out = b.flush();
        assert_eq!(out, vec!["One sentence remaining no terminator"]);
    }

    #[test]
    fn flush_is_empty_when_buffer_is_empty() {
        let mut b = SentenceBuffer::new();
        let out = b.feed("Hi there.");
        assert_eq!(out, vec!["Hi there."]);
        assert!(b.flush().is_empty());
    }

    #[test]
    fn handles_emdash_and_questions_together() {
        let mut b = SentenceBuffer::new();
        let out = b.feed("Wait — really? Yes!");
        assert_eq!(out, vec!["Wait — really?", "Yes!"]);
    }
}
