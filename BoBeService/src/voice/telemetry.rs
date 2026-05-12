//! Voice SLO telemetry — Prometheus exposition served at `GET /metrics`.
//!
//! Histograms cover the per-stage latency budgets from `docs/voice-plan.md`
//! D10: capture, VAD, STT first partial, smart-turn inference, set_model
//! RPC, LLM TTFT, sentence emission, TTS TTFB, opus encode, e2e. Counters
//! cover discrete events: barge-in success/false, filler triggers,
//! segment-drop backpressure, cancel-phrase matches, ask_user gates.
//!
//! Records flow through the `metrics` facade (`metrics::histogram!`,
//! `metrics::counter!`), which the installed `PrometheusBuilder` recorder
//! turns into a static text exposition via `PrometheusHandle::render`.

use metrics::{Unit, describe_counter, describe_histogram};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};

// Latency histograms — milliseconds across the voice turn lifecycle.
// Only the histograms that have actual `.record()` callsites are described
// here; adding describes for un-recorded histograms pollutes the
// Prometheus exposition with empty series. New stages get their describe
// added when their record callsite goes in.
pub(crate) const HIST_STT_MS: &str = "voice_stt_ms";
pub(crate) const HIST_SMART_TURN_MS: &str = "voice_smart_turn_inference_ms";
pub(crate) const HIST_E2E_MS: &str = "voice_e2e_ms";

// Event counters — discrete signals.
pub(crate) const CTR_TURN_COMPLETE: &str = "voice_turn_complete_total";
pub(crate) const CTR_TURN_ERROR: &str = "voice_turn_error_total";
pub(crate) const CTR_BARGE_IN_SUCCESS: &str = "voice_barge_in_success_total";
pub(crate) const CTR_BARGE_IN_FALSE: &str = "voice_barge_in_false_total";
pub(crate) const CTR_FILLER_TRIGGER: &str = "voice_filler_trigger_total";
pub(crate) const CTR_SEGMENT_DROP: &str = "voice_segment_backpressure_drop_total";
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

    describe_histogram!(HIST_STT_MS, Unit::Milliseconds, "STT segment transcribe time");
    describe_histogram!(
        HIST_SMART_TURN_MS,
        Unit::Milliseconds,
        "Smart-turn inference time on the last speech segment"
    );
    describe_histogram!(
        HIST_E2E_MS,
        Unit::Milliseconds,
        "End-of-speech to first audio frame on the wire (composite SLO)"
    );

    describe_counter!(CTR_TURN_COMPLETE, Unit::Count, "Voice turns that ran to completion");
    describe_counter!(CTR_TURN_ERROR, Unit::Count, "Voice turns aborted by an error path");
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
        CTR_SEGMENT_DROP,
        Unit::Count,
        "Speech segments dropped because a turn was already in flight"
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
