//! Voice SLO telemetry — Prometheus at `GET /metrics`. Histograms = per-
//! stage latencies (docs/physical-bobe.md); counters = discrete events
//! (barge-in, fillers, cancel-phrase, ask_user). Uses the `metrics`
//! facade → `PrometheusBuilder` recorder → `PrometheusHandle::render`.

use metrics::{Unit, describe_counter, describe_histogram};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};

// Latency histograms — milliseconds across the voice turn lifecycle.
// Only the histograms that have actual `.record()` callsites are described
// here; adding describes for un-recorded histograms pollutes the
// Prometheus exposition with empty series. New stages get their describe
// added when their record callsite goes in.
pub(crate) const HIST_E2E_MS: &str = "voice_transcript_to_first_audio_ms";
pub(crate) const HIST_TURN_TOTAL_MS: &str = "voice_turn_total_ms";
pub(crate) const HIST_LLM_TTFT_MS: &str = "voice_llm_ttft_ms";
pub(crate) const HIST_FIRST_SENTENCE_MS: &str = "voice_first_sentence_ms";
pub(crate) const HIST_TTS_SYNTH_MS: &str = "voice_tts_synth_ms";

// Event counters — discrete signals.
pub(crate) const CTR_TURN_COMPLETE: &str = "voice_turn_complete_total";
pub(crate) const CTR_BARGE_IN_SUCCESS: &str = "voice_barge_in_success_total";
pub(crate) const CTR_BARGE_IN_FALSE: &str = "voice_barge_in_false_total";
pub(crate) const CTR_FILLER_TRIGGER: &str = "voice_filler_trigger_total";
pub(crate) const CTR_CANCEL_PHRASE: &str = "voice_cancel_phrase_match_total";
pub(crate) const CTR_ASK_USER_BLOCKED: &str = "voice_ask_user_blocked_total";

/// Install the Prometheus recorder and register descriptions for every
/// metric the voice pipeline emits. Returns a handle whose `.render()`
/// produces the exposition text the `/metrics` HTTP route serves. Call
/// once at bootstrap before any voice traffic; later `install_recorder`
/// calls fail loudly (the global recorder slot is already taken).
pub(crate) fn install_recorder() -> Result<PrometheusHandle, String> {
    let handle = PrometheusBuilder::new()
        .install_recorder()
        .map_err(|e| format!("prometheus install: {e}"))?;

    describe_histogram!(
        HIST_E2E_MS,
        Unit::Milliseconds,
        "Final transcript arrival to first audio frame on the wire"
    );
    describe_histogram!(
        HIST_TURN_TOTAL_MS,
        Unit::Milliseconds,
        "Final transcript arrival to completion of the spoken turn"
    );
    describe_histogram!(
        HIST_LLM_TTFT_MS,
        Unit::Milliseconds,
        "Chat worker send to first assistant text delta"
    );
    describe_histogram!(
        HIST_FIRST_SENTENCE_MS,
        Unit::Milliseconds,
        "Final transcript arrival to first speakable sentence boundary"
    );
    describe_histogram!(
        HIST_TTS_SYNTH_MS,
        Unit::Milliseconds,
        "Per-sentence text-to-speech synthesis duration"
    );

    describe_counter!(
        CTR_TURN_COMPLETE,
        Unit::Count,
        "Voice turns that ran to completion"
    );
    describe_counter!(
        CTR_BARGE_IN_SUCCESS,
        Unit::Count,
        "Barge-in fires that passed gating and truncated a turn"
    );
    describe_counter!(
        CTR_BARGE_IN_FALSE,
        Unit::Count,
        "Barge-in fires dropped by MinWords or similar gating"
    );
    describe_counter!(
        CTR_FILLER_TRIGGER,
        Unit::Count,
        "Cached filler PCM emissions (TTFT watchdog + per-tool + error fillers)"
    );
    describe_counter!(
        CTR_CANCEL_PHRASE,
        Unit::Count,
        "Streaming-STT partials that matched a cancel-phrase regex"
    );
    describe_counter!(
        CTR_ASK_USER_BLOCKED,
        Unit::Count,
        "ask_user tool calls denied because the turn was in voice mode"
    );

    Ok(handle)
}
