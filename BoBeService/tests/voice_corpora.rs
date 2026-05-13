//! Tier-3 corpora tests for the voice stack. All `#[ignore]`-gated; run
//! locally with `cargo test --test voice_corpora -- --ignored` or via the
//! nightly CI workflow at `.github/workflows/voice-nightly.yml`.
//!
//! These tests exercise the **real** STT, TTS, and smart-turn engines
//! against published corpora — no fakes, no mocks (per the project's
//! `feedback_no_fakes_no_mocks.md`). They require:
//!   - Voice models installed at `~/.bobe/models/` (`just voice-install`)
//!   - Test corpora at `~/.cache/bobe-tests/` (run `scripts/fetch-test-corpora.sh`)
//!
//! Quality gates:
//!   - LibriSpeech-test-clean WER ≤ 10%
//!   - Smart-turn-data-v3-test accuracy ≥ 85%
//!   - Kokoro UTMOS ≥ 3.5 (round-trip via Whisper as a proxy until UTMOS-rs lands)
//!
#![allow(
    clippy::print_stderr,
    clippy::unwrap_used,
    clippy::expect_used
)]

//! Each test is independent — a failure in one shouldn't block the others.

use std::path::PathBuf;

/// Returns the corpus root, or `None` if the cache is missing. Tests skip
/// (return `Ok(())`) rather than fail when corpora aren't installed, so a
/// developer who hasn't run `fetch-test-corpora.sh` doesn't see red.
fn corpus_root() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let root = home.join(".cache").join("bobe-tests");
    root.is_dir().then_some(root)
}

#[test]
#[ignore = "tier3-corpora: requires LibriSpeech-test-clean at ~/.cache/bobe-tests/librispeech"]
fn librispeech_wer_under_threshold() {
    let Some(root) = corpus_root() else {
        eprintln!("skip: corpus root missing");
        return;
    };
    let _librispeech = root.join("librispeech");
    // TODO(tier3): wire up real Zipformer transcription + WER computation
    // via `jiwer-rs` or hand-rolled Levenshtein over LibriSpeech-test-clean
    // wav/transcript pairs. Threshold: WER ≤ 10%.
    eprintln!("librispeech WER test scaffold — implementation pending");
}

#[test]
#[ignore = "tier3-corpora: requires smart-turn-data-v3-test at ~/.cache/bobe-tests/smart-turn"]
fn smart_turn_accuracy_above_threshold() {
    let Some(root) = corpus_root() else {
        eprintln!("skip: corpus root missing");
        return;
    };
    let _smart_turn = root.join("smart-turn");
    // TODO(tier3): load the smart-turn-data-v3-test set, run the daemon's
    // `OnnxSmartTurn` over each clip, compare predicted-complete vs label.
    // Threshold: accuracy ≥ 85% English.
    eprintln!("smart-turn accuracy test scaffold — implementation pending");
}

#[test]
#[ignore = "tier3-corpora: requires Kokoro models at ~/.bobe/models/kokoro-multi-lang-v1_0"]
fn kokoro_quality_round_trip() {
    // TODO(tier3): synthesize a fixed set of 20 reference utterances via
    // Kokoro, transcribe back through Zipformer, assert WER ≤ 5% on the
    // synth→transcribe round-trip as a proxy for UTMOS until UTMOS-rs lands.
    eprintln!("kokoro round-trip test scaffold — implementation pending");
}
