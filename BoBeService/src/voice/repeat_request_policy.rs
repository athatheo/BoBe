//! Hammering-pushback dedup helper. Tracks recent user requests and decides
//! whether a new request is "near-identical" to the previous one — the signal
//! the proactive voice path uses to escalate cadence (M5.2).
//!
//! Levenshtein < 0.3 of the longer string = "rephrase". Within a 60-second
//! window, repeated near-identical requests are counted; the count drives a
//! 4-level escalation:
//!   0  → silent enqueue (first time)
//!   1  → soft acknowledge  ("On it.")
//!   2  → firm pushback     ("I'm working on it. One sec.")
//!   3+ → persona-specific abort+merge
//!
//! Today this module owns the detection only; the policy mapping lives with
//! the proactive voice route in `voice/proactive.rs`.

use std::sync::Mutex;
use std::time::{Duration, Instant};

const HAMMERING_WINDOW: Duration = Duration::from_mins(1);
/// Levenshtein ratio (`distance / max(len_a, len_b)`) below which two
/// strings are considered the same request rephrased. Empirically chosen;
/// sits between Pipecat's 0.2 (strict) and OpenAI Realtime's 0.4 (loose).
const REPHRASE_THRESHOLD: f32 = 0.3;

/// Rolling state for one conversation. Cheap to construct; lives behind a
/// `Mutex` because the proactive path is async but the detector is sync.
#[allow(dead_code)] // wired into proactive policy in M5.2
pub(crate) struct HammeringDetector {
    state: Mutex<DetectorState>,
}

#[derive(Default)]
struct DetectorState {
    last_request: Option<String>,
    last_at: Option<Instant>,
    consecutive_rephrases: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // levels consumed by M5.2 escalation table
pub(crate) enum HammeringLevel {
    /// First-time or cooled-down request. Proactive path enqueues silently.
    None,
    /// One repeat within the window. Soft acknowledge.
    Soft,
    /// Two repeats. Firm pushback.
    Firm,
    /// Three+ repeats. Persona-specific escalation; cancel-and-merge.
    Escalate,
}

impl Default for HammeringDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)] // wired into proactive policy + M5.2 escalation
impl HammeringDetector {
    pub(crate) fn new() -> Self {
        Self {
            state: Mutex::new(DetectorState::default()),
        }
    }

    /// Record a new user request and return the escalation level for it.
    /// Resets when the inter-request gap exceeds `HAMMERING_WINDOW` so a
    /// long pause clears the count.
    pub(crate) fn observe(&self, request: &str) -> HammeringLevel {
        let mut s = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let now = Instant::now();

        let cooled = s
            .last_at
            .is_some_and(|prev| now.saturating_duration_since(prev) > HAMMERING_WINDOW);
        if cooled {
            s.consecutive_rephrases = 0;
        }

        let is_rephrase = s
            .last_request
            .as_deref()
            .is_some_and(|prev| levenshtein_ratio(prev, request) < REPHRASE_THRESHOLD);

        if is_rephrase {
            s.consecutive_rephrases = s.consecutive_rephrases.saturating_add(1);
        } else {
            s.consecutive_rephrases = 0;
        }
        s.last_request = Some(request.to_owned());
        s.last_at = Some(now);

        match s.consecutive_rephrases {
            0 => HammeringLevel::None,
            1 => HammeringLevel::Soft,
            2 => HammeringLevel::Firm,
            _ => HammeringLevel::Escalate,
        }
    }

    /// Force-clear the detector state — call on conversation close so a new
    /// conversation doesn't inherit the previous one's hammering count.
    #[allow(dead_code)] // used by future conversation-close hook
    pub(crate) fn reset(&self) {
        let mut s = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *s = DetectorState::default();
    }
}

/// Levenshtein distance / max(len) — 0.0 means identical, 1.0 means
/// completely different. Used as a rephrase-detection heuristic.
fn levenshtein_ratio(a: &str, b: &str) -> f32 {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let n = a_chars.len();
    let m = b_chars.len();
    if n == 0 && m == 0 {
        return 0.0;
    }
    let max = n.max(m) as f32;
    levenshtein(&a_chars, &b_chars) as f32 / max
}

fn levenshtein(a: &[char], b: &[char]) -> usize {
    let n = a.len();
    let m = b.len();
    if n == 0 {
        return m;
    }
    if m == 0 {
        return n;
    }
    // Two-row DP. m+1 cells; reuse to keep allocations tiny.
    let mut prev: Vec<usize> = (0..=m).collect();
    let mut curr: Vec<usize> = vec![0; m + 1];
    for i in 1..=n {
        curr[0] = i;
        for j in 1..=m {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            curr[j] = (curr[j - 1] + 1)
                .min(prev[j] + 1)
                .min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[m]
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
    use super::*;

    #[test]
    fn levenshtein_ratio_identical_is_zero() {
        assert_eq!(levenshtein_ratio("hello", "hello"), 0.0);
    }

    #[test]
    fn levenshtein_ratio_disjoint_is_one() {
        assert_eq!(levenshtein_ratio("abc", "xyz"), 1.0);
    }

    #[test]
    fn levenshtein_ratio_minor_edit_below_threshold() {
        // "what is the time" vs "whats the time" — one edit difference.
        let r = levenshtein_ratio("what is the time", "whats the time");
        assert!(r < REPHRASE_THRESHOLD, "expected rephrase, got {r}");
    }

    #[test]
    fn detector_first_request_returns_none() {
        let d = HammeringDetector::new();
        assert_eq!(d.observe("can you help me"), HammeringLevel::None);
    }

    #[test]
    fn detector_repeat_escalates() {
        let d = HammeringDetector::new();
        assert_eq!(d.observe("what's the weather"), HammeringLevel::None);
        assert_eq!(d.observe("whats the weather"), HammeringLevel::Soft);
        assert_eq!(d.observe("what's the weather!"), HammeringLevel::Firm);
        assert_eq!(d.observe("whats the weather"), HammeringLevel::Escalate);
    }

    #[test]
    fn detector_distinct_requests_dont_escalate() {
        let d = HammeringDetector::new();
        assert_eq!(d.observe("set a timer"), HammeringLevel::None);
        assert_eq!(d.observe("what time is it"), HammeringLevel::None);
        assert_eq!(d.observe("turn on the lights"), HammeringLevel::None);
    }
}
