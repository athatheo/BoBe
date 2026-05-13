//! Proactive voice routing scaffold (M5.2). Synthesises a piece of text via
//! Kokoro and pushes the resulting Opus frames over the active voice sink
//! without going through the user-message turn pipeline. Used by the
//! hammering-pushback path (`HammeringDetector` → soft / firm / escalate
//! pushback) and any future "BoBe checks in unprompted" feature.
//!
//! Architecture: this module owns the *route* (sink push + opus encode);
//! the *policy* (when to fire, what to say) lives in callers. Single-flight
//! is enforced by `RuntimeSession::try_begin_proactive_message` so a
//! proactive synthesis can't interleave with a user turn.

use std::sync::Arc;

use tracing::{info, warn};

use crate::runtime::session::RuntimeSession;
use crate::speech::TtsEngine;
use crate::speech::protocol::{FLAG_FIRST_OF_TURN, encode_tts_frame};
use crate::voice::opus::{encode_pcm_with, make_opus_encoder};
use crate::voice::sinks::VoiceSink;

/// Synthesise `text` using the daemon's TTS and push it over the active
/// voice sink. Returns `Ok(())` on a successful end-to-end push or when
/// no client is connected (silently no-op — proactive work shouldn't error
/// just because nobody is listening).
///
/// `runtime_session` is consulted for the proactive single-flight guard.
/// If a user turn is in flight the call returns `Err(...)`; callers can
/// log + skip or queue for retry.
#[allow(dead_code)] // wired into M5.2 hammering pushback + scheduled checkin
pub(crate) async fn route_to_voice_sink(
    runtime_session: &RuntimeSession,
    voice_sink: &VoiceSink,
    tts: &Arc<dyn TtsEngine>,
    voice_id: &str,
    speed: f32,
    text: &str,
) -> Result<(), String> {
    let _guard = runtime_session
        .try_begin_proactive_message()
        .map_err(|e| format!("proactive admission denied: {e}"))?;

    // Skip the synthesis altogether if no client is listening — saves
    // ~200ms of Kokoro work on the cold-start case.
    let Some(sink_tx) = voice_sink.get().await else {
        info!("voice.proactive_no_client_skip");
        return Ok(());
    };

    let tts_clone = Arc::clone(tts);
    let voice_owned = voice_id.to_string();
    let text_owned = text.to_string();
    let sample_rate = tts.sample_rate();
    let synth = tokio::task::spawn_blocking(move || {
        tts_clone.synthesize(&text_owned, &voice_owned, speed)
    })
    .await;

    let pcm = match synth {
        Ok(Ok(pcm)) => pcm,
        Ok(Err(e)) => return Err(format!("tts synthesis failed: {e}")),
        Err(e) => return Err(format!("tts join failure: {e}")),
    };

    let Some(mut encoder) = make_opus_encoder(sample_rate) else {
        return Err("opus encoder init failed".into());
    };

    let chunk_id = rand_chunk_id();
    let frames = encode_pcm_with(&mut encoder, &pcm, sample_rate);
    if frames.is_empty() {
        return Err("opus produced zero frames".into());
    }
    for (idx, opus) in frames.into_iter().enumerate() {
        let flags = if idx == 0 { FLAG_FIRST_OF_TURN } else { 0 };
        let payload = encode_tts_frame(chunk_id, flags, &opus);
        if sink_tx
            .send(axum::extract::ws::Message::Binary(payload.into()))
            .await
            .is_err()
        {
            warn!("voice.proactive_sink_send_failed");
            break;
        }
    }
    Ok(())
}

/// Cheap unique chunk id for proactive frames. Doesn't need to coordinate
/// with the per-turn ids; clients only use chunk_id for ordering within a
/// message which proactive doesn't span.
fn rand_chunk_id() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64)
}
