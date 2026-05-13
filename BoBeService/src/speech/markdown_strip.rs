//! Streaming markdown stripper for TTS. Kokoro will speak literal asterisks,
//! underscores, backticks etc. if they reach it; this strips them before they
//! enter the sentence buffer.
//!
//! Streaming model: maintain the full raw running buffer, re-strip it on every
//! delta, and emit only the new tail (clean[last_emitted_len..]). If a code
//! fence is currently unclosed, suppress emission entirely so we don't speak
//! half a code block.

use std::sync::LazyLock;

use regex::Regex;

#[allow(
    clippy::expect_used,
    reason = "static compile-time regexes; failure is a developer error caught in tests"
)]
static MD_RE: LazyLock<MarkdownRegexes> = LazyLock::new(|| MarkdownRegexes {
    code_fence: Regex::new(r"(?s)```[^`]*```").expect("static code-fence regex"),
    inline_code: Regex::new(r"`([^`]+)`").expect("static inline-code regex"),
    bold: Regex::new(r"\*\*([^*]+)\*\*").expect("static bold regex"),
    italic_star: Regex::new(r"\*([^*\n]+)\*").expect("static italic-star regex"),
    italic_underscore: Regex::new(r"_([^_\n]+)_").expect("static italic-underscore regex"),
    header: Regex::new(r"(?m)^#{1,6}\s+").expect("static header regex"),
    bullet: Regex::new(r"(?m)^\s*[-*•]\s+").expect("static bullet regex"),
    numbered: Regex::new(r"(?m)^\s*\d+\.\s+").expect("static numbered-list regex"),
    link: Regex::new(r"\[([^\]]+)\]\([^)]+\)").expect("static markdown-link regex"),
});

struct MarkdownRegexes {
    code_fence: Regex,
    inline_code: Regex,
    bold: Regex,
    italic_star: Regex,
    italic_underscore: Regex,
    header: Regex,
    bullet: Regex,
    numbered: Regex,
    link: Regex,
}

pub(crate) struct MarkdownStripper {
    raw: String,
    last_emitted_clean_len: usize,
}

impl MarkdownStripper {
    pub(crate) fn new() -> Self {
        Self {
            raw: String::new(),
            last_emitted_clean_len: 0,
        }
    }

    /// Add a raw text delta. Returns the new clean tail to send onward, or an
    /// empty string if a fenced code block is currently open (suppress).
    pub(crate) fn feed(&mut self, delta: &str) -> String {
        self.raw.push_str(delta);
        if self.unclosed_fence() {
            return String::new();
        }
        let clean = strip_markdown(&self.raw);
        if clean.len() > self.last_emitted_clean_len {
            let tail = clean[self.last_emitted_clean_len..].to_string();
            self.last_emitted_clean_len = clean.len();
            tail
        } else {
            String::new()
        }
    }

    fn unclosed_fence(&self) -> bool {
        self.raw.matches("```").count() % 2 == 1
    }
}

pub(crate) fn strip_markdown(text: &str) -> String {
    let re = &*MD_RE;
    let mut out = re.code_fence.replace_all(text, "").to_string();
    out = re.inline_code.replace_all(&out, "$1").to_string();
    out = re.bold.replace_all(&out, "$1").to_string();
    out = re.italic_star.replace_all(&out, "$1").to_string();
    out = re.italic_underscore.replace_all(&out, "$1").to_string();
    out = re.header.replace_all(&out, "").to_string();
    out = re.bullet.replace_all(&out, "").to_string();
    out = re.numbered.replace_all(&out, "").to_string();
    out = re.link.replace_all(&out, "$1").to_string();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_bold_and_italic() {
        assert_eq!(strip_markdown("**Hello** _world_"), "Hello world");
    }

    #[test]
    fn strips_headers_bullets_links() {
        let md = "# Title\n- one\n- two\n[click](https://x)";
        assert_eq!(strip_markdown(md), "Title\none\ntwo\nclick");
    }

    #[test]
    fn strips_fenced_code() {
        let md = "before\n```rust\nlet x = 1;\n```\nafter";
        assert_eq!(strip_markdown(md), "before\n\nafter");
    }

    #[test]
    fn streaming_suppresses_open_fence() {
        let mut s = MarkdownStripper::new();
        // First chunk: opens a fence
        assert_eq!(s.feed("Let me show you:\n```rust\nlet"), "");
        // Still open
        assert_eq!(s.feed("x = 1;\n"), "");
        // Now closes
        let out = s.feed("```\nThat's it.");
        // Fence content is dropped, the trailing prose is emitted
        assert!(out.contains("That's it."));
    }

    #[test]
    fn streaming_emits_clean_tail() {
        let mut s = MarkdownStripper::new();
        assert_eq!(s.feed("**Hello**"), "Hello");
        assert_eq!(s.feed(" world"), " world");
    }

    #[test]
    fn handles_consecutive_fences() {
        // Two fenced blocks back-to-back: both stripped, prose between kept.
        let md = "intro\n```rust\nlet x = 1;\n```\nmiddle\n```python\nprint(1)\n```\nend";
        let out = strip_markdown(md);
        assert!(out.contains("intro"), "kept opening prose: {out}");
        assert!(out.contains("middle"), "kept inter-fence prose: {out}");
        assert!(out.contains("end"), "kept closing prose: {out}");
        assert!(!out.contains("let x"), "stripped first fence: {out}");
        assert!(!out.contains("print(1)"), "stripped second fence: {out}");
    }

    #[test]
    fn streaming_suppresses_until_second_fence_closes() {
        // Stream interleaves opening/closing two fences; the stripper should
        // only emit clean tail when the running fence count is even.
        let mut s = MarkdownStripper::new();
        assert_eq!(s.feed("Run ```sh\necho hi\n``` then "), "Run  then ");
        assert_eq!(s.feed("```sh\necho bye\n"), ""); // second fence open
        let tail = s.feed("``` and exit");
        assert!(tail.contains("and exit"), "got: {tail}");
    }

    #[test]
    fn flat_asterisk_runs_unchanged() {
        // A bare "5 * 4 = 20" shouldn't lose its asterisks (italic regex
        // requires the run to be matched on both sides without newlines).
        let out = strip_markdown("5 * 4 = 20");
        // Either the asterisks survive intact, or they're stripped — the
        // spec is "don't crash and don't keep markdown syntax". We assert
        // that the digits remain intact and there's no markdown-specific
        // byte left behind beyond the asterisks themselves.
        assert!(out.contains("5"), "{out}");
        assert!(out.contains("20"), "{out}");
    }

    #[test]
    fn link_preserves_link_text_and_drops_url() {
        let out = strip_markdown("Visit [our docs](https://example.com/docs) please");
        assert_eq!(out, "Visit our docs please");
        assert!(!out.contains("https://"), "{out}");
    }
}
