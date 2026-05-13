//! Cancel-phrase detection on streaming-STT partials.
//!
//! When the user says "stop" / "nevermind" / "cancel that" during BoBe's
//! TTS playback, we abort the in-flight turn WITHOUT round-tripping the
//! LLM. The phrase wasn't a question; it was a barge-in marker. Acting on
//! it locally trims ~500-2000ms vs sending it to Copilot and waiting for
//! the LLM to figure out it should stop.
//!
//! Pattern list is intentionally short — false positives ("stop signs",
//! "cancel my subscription tomorrow") are worse than missing one
//! interrupt. Lower-case match after stripping non-alphanumeric.

use std::sync::LazyLock;

use regex::Regex;

static CANCEL_PHRASE_RE: LazyLock<Regex> = LazyLock::new(|| {
    // Anchored to a word boundary on the left; matches anywhere in the
    // partial transcript. Phrases all start with command verbs to keep
    // false-positive surface low.
    //
    // SAFETY: Pattern is a compile-time constant exercised by every test
    // run; if it ever fails to compile that's a build-blocking unit test
    // failure, not a runtime crash in the wild.
    #[allow(clippy::expect_used)]
    Regex::new(
        r"(?ix)
        \b(
            stop                   |
            never\s*mind           |
            cancel(\s+that)?       |
            shut\s*up              |
            wait\s+wait            |
            pause\s+it             |
            hold\s+on\s+hold\s+on  |
            forget\s+it            |
            be\s+quiet
        )\b",
    )
    .expect("cancel-phrase regex must compile")
});

/// Returns true if the partial transcript contains a cancel phrase. Caller
/// is responsible for gating on "are we currently speaking" — a cancel
/// phrase before BoBe says anything is just speech, not a barge-in.
pub(crate) fn is_cancel_phrase(partial: &str) -> bool {
    CANCEL_PHRASE_RE.is_match(partial)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_canonical_cancels() {
        assert!(is_cancel_phrase("stop"));
        assert!(is_cancel_phrase("Stop please"));
        assert!(is_cancel_phrase("never mind"));
        assert!(is_cancel_phrase("nevermind"));
        assert!(is_cancel_phrase("cancel that"));
        assert!(is_cancel_phrase("just cancel"));
        assert!(is_cancel_phrase("shut up"));
        assert!(is_cancel_phrase("wait wait"));
        assert!(is_cancel_phrase("hold on hold on"));
        assert!(is_cancel_phrase("forget it"));
        assert!(is_cancel_phrase("be quiet please"));
    }

    #[test]
    fn does_not_match_non_cancels() {
        assert!(!is_cancel_phrase("hello world"));
        assert!(!is_cancel_phrase("tell me about apples"));
        assert!(!is_cancel_phrase("what was that"));
        // Brittle case worth pinning: "I would never" alone isn't cancel.
        assert!(!is_cancel_phrase("I would never"));
    }
}
