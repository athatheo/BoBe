//! Voice SLO telemetry — Prometheus at `GET /metrics`. Histograms = per-
//! stage latencies (docs/voice-plan.md D10); counters = discrete events
//! (barge-in, fillers, cancel-phrase, ask_user). Uses the `metrics`
//! facade → `PrometheusBuilder` recorder → `PrometheusHandle::render`.

use metrics::{Unit, describe_counter, describe_histogram};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};

// Latency histograms — milliseconds across the voice turn lifecycle.
// Only the histograms that have actual `.record()` callsites are described
// here; adding describes for un-recorded histograms pollutes the
// Prometheus exposition with empty series. New stages get their describe
// added when their record callsite goes in.
pub(crate) const HIST_E2E_MS: &str = "voice_e2e_ms";

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
        "End-of-speech to first audio frame on the wire (composite SLO)"
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
