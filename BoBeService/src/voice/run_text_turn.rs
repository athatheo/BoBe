//! Shared convergence helper for both voice variants.
//!
//! Mode A (server-side STT): `voice/turn_flow.rs::process_turn` runs the
//! smart-turn gate + Zipformer commit, then calls `run_text_turn`.
//! Mode B (client-side STT): `voice/modes/transcript_in.rs` receives a
//! `transcript.final` from the WS and calls `run_text_turn` directly.
//!
//! The function owns single-flight admission, Thinking/Speaking/Listening
//! state emission, the per-turn TTS pipeline (filler + Kokoro), and the
//! LLM observer that feeds `SentencePipeline`. The convergence point —
//! `RuntimeSession::handle_user_message_with_observer` — lives at line ~80
//! below; nothing about how the text arrived is visible from there down.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use axum::extract::ws::Message;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, error, warn};

use crate::speech::TtsEngine;
use crate::speech::protocol::{
    FLAG_FILLER, FLAG_FIRST_OF_TURN, ServerMessage, VoicePhase, encode_tts_frame,
};
use crate::voice::context::VoiceContext;
use crate::voice::filler_library::FillerKind;
use crate::voice::opus::{encode_pcm_with, make_opus_encoder};
use crate::voice::protocol_helpers::{send_error, send_json, send_state};
use crate::voice::sentence_pipeline::SentencePipeline;
use crate::voice::session::SessionVoiceConfig;
use crate::voice::telemetry::{CTR_FILLER_TRIGGER, CTR_TURN_COMPLETE, HIST_E2E_MS};

/// Time-to-first-audio budget. If the Kokoro task hasn't pushed any audio
/// frames by this deadline (the LLM is slow or the first sentence is still
/// buffering), the filler watchdog emits the pre-rendered cached phrase.
/// Matches LiveKit/Pipecat/ElevenLabs production threshold.
pub(crate) const FILLER_TRIGGER: Duration = Duration::from_millis(800);

/// mpsc capacity between the LLM-observer (sync closure) and the Kokoro
/// task. Bursts during fast LLM token rates can push 3-5 sentences within
/// ~200ms; 16 gives comfortable headroom while still bounding memory.
pub(crate) const SENTENCE_CHANNEL_CAPACITY: usize = 16;

/// RAII flag: `voice_turn_active` is set to true on construction and cleared
/// on drop. Ensures the BobeHooks signal goes back to false even if the
/// run_text_turn body errors out mid-flight.
struct VoiceTurnFlag(Arc<AtomicBool>);

impl Drop for VoiceTurnFlag {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// The convergence body. Both Mode A and Mode B reach this with a
/// pre-validated non-empty trimmed `text`. Caller is responsible for any
/// pre-text work (smart-turn gate, STT commit, empty-transcript handling)
/// and for populating the session's `current_turn` slot if barge-in must
/// be honored.
///
/// **M6.A ordering shift:** in the pre-refactor flow, Mode A emitted
/// `state(Thinking)` BEFORE STT and rolled back to `state(Listening)` on
/// STT failure / empty transcript. Post-refactor, STT runs first
/// (`voice/turn_flow.rs::process_turn`) and only successful commits reach
/// here, so `Thinking` is emitted after the transcript exists. The
/// observable delta: STT error/empty no longer flashes a Thinking pulse
/// before the error. Acceptable — clients handle out-of-order states.
pub(crate) async fn run_text_turn(
    text: &str,
    turn_id: &str,
    ctx: &VoiceContext,
    voice_cfg: SessionVoiceConfig,
) {
    let engines = &ctx.engines;
    let out_tx = &ctx.out_tx;
    let runtime_session = &ctx.runtime_session;
    let turn_start = Instant::now();

    let guard = match runtime_session.try_begin_user_message() {
        Ok(g) => g,
        Err(reason) => {
            send_error(out_tx, "conflict", reason).await;
            // Roll the wire state back to Listening so the client UI doesn't
            // get stuck mid-turn (relevant when Mode B has pre-emitted
            // Thinking before calling here; Mode A hasn't emitted anything
            // beyond the ambient state at this point but the explicit
            // Listening is harmless).
            send_state(out_tx, VoicePhase::Listening, turn_id).await;
            return;
        }
    };

    // Flag set AFTER single-flight admission — otherwise a microsecond
    // window between flag-set and try_begin_user_message failure could let
    // another worker class's UserPromptSubmitted hook see voice_mode=true
    // and inject the voice-tone hint into a non-voice turn. RAII guard
    // clears on drop including panic-unwind.
    ctx.voice_turn_active.store(true, Ordering::Release);
    let _voice_flag = VoiceTurnFlag(Arc::clone(&ctx.voice_turn_active));

    send_state(out_tx, VoicePhase::Thinking, turn_id).await;
    send_json(
        out_tx,
        &ServerMessage::TranscriptFinal {
            turn_id: turn_id.to_string(),
            text: text.to_string(),
        },
    )
    .await;

    send_state(out_tx, VoicePhase::Speaking, turn_id).await;
    let speaking_start = Instant::now();
    let first_audio_emitted = Arc::new(AtomicBool::new(false));
    let filler_task = spawn_filler_watchdog(
        engines
            .fillers
            .as_ref()
            .and_then(|lib| lib.get(FillerKind::Thinking)),
        engines.tts.sample_rate(),
        Arc::clone(&first_audio_emitted),
        out_tx.clone(),
    );
    let (sentence_tx, sentence_rx) = mpsc::channel::<String>(SENTENCE_CHANNEL_CAPACITY);
    let kokoro_task = spawn_kokoro_task(
        Arc::clone(&engines.tts),
        sentence_rx,
        out_tx.clone(),
        Arc::clone(&first_audio_emitted),
        voice_cfg.clone(),
    );

    let pipeline = Arc::new(std::sync::Mutex::new(SentencePipeline::new(
        sentence_tx.clone(),
    )));
    let pipeline_for_obs = Arc::clone(&pipeline);
    let observer = move |delta: &str| {
        if let Ok(mut p) = pipeline_for_obs.lock() {
            p.feed(delta);
        }
    };

    runtime_session
        .handle_user_message_with_observer(text, turn_id, observer)
        .await;

    if let Ok(mut p) = pipeline.lock() {
        p.flush();
    }
    drop(pipeline);
    drop(sentence_tx);
    match kokoro_task.await {
        Ok(()) => {}
        Err(e) if e.is_panic() => {
            error!(error = %e, "voice.kokoro_task_panic");
        }
        Err(e) if e.is_cancelled() => {
            debug!("voice.kokoro_task_cancelled");
        }
        Err(e) => warn!(error = %e, "voice.kokoro_task_join_failed"),
    }

    if let Some(handle) = filler_task {
        handle.abort();
        drop(handle.await);
    }

    send_json(
        out_tx,
        &ServerMessage::TtsEnd {
            turn_id: turn_id.to_string(),
        },
    )
    .await;
    send_state(out_tx, VoicePhase::Listening, turn_id).await;
    drop(guard);

    let total_ms = turn_start.elapsed().as_millis() as u64;
    metrics::histogram!(HIST_E2E_MS).record(total_ms as f64);
    metrics::counter!(CTR_TURN_COMPLETE).increment(1);
    tracing::info!(
        speaking_ms = speaking_start.elapsed().as_millis() as u64,
        total_ms,
        "voice.turn_complete"
    );
}

fn spawn_kokoro_task(
    tts: Arc<dyn TtsEngine>,
    mut sentence_rx: mpsc::Receiver<String>,
    out_tx: mpsc::Sender<Message>,
    first_audio_emitted: Arc<AtomicBool>,
    voice_cfg: SessionVoiceConfig,
) -> tokio::task::JoinHandle<()> {
    let sample_rate = tts.sample_rate();
    tokio::spawn(async move {
        let Some(mut encoder) = make_opus_encoder(sample_rate) else {
            return;
        };
        let mut chunk_id: u64 = 0;
        let mut first_chunk_pending = true;
        while let Some(sentence) = sentence_rx.recv().await {
            let tts_clone = Arc::clone(&tts);
            let sentence_owned = sentence.clone();
            let voice_id = voice_cfg.voice_id.clone();
            let speed = voice_cfg.speed;
            let synth = tokio::task::spawn_blocking(move || {
                tts_clone.synthesize(&sentence_owned, &voice_id, speed)
            })
            .await;
            let pcm = match synth {
                Ok(Ok(pcm)) => pcm,
                Ok(Err(e)) => {
                    warn!(error = %e, "voice.kokoro_synth_failed");
                    continue;
                }
                Err(e) => {
                    warn!(error = %e, "voice.kokoro_join_failed");
                    continue;
                }
            };
            for opus_packet in encode_pcm_with(&mut encoder, &pcm, sample_rate) {
                let mut flags = 0_u8;
                if first_chunk_pending {
                    flags |= FLAG_FIRST_OF_TURN;
                    first_chunk_pending = false;
                    first_audio_emitted.swap(true, Ordering::AcqRel);
                }
                let framed = encode_tts_frame(chunk_id, flags, &opus_packet);
                chunk_id = chunk_id.saturating_add(1);
                if out_tx.send(Message::Binary(framed.into())).await.is_err() {
                    return;
                }
            }
        }
    })
}

/// 800ms TTFT filler watchdog. Sleeps then races the Kokoro task for the
/// "first audio emitted" flag. If we win (Kokoro hasn't produced anything
/// yet), encode the pre-rendered filler PCM as Opus 20ms frames and stream
/// them with `FLAG_FILLER` set so the client knows they're preemptible.
/// Returns `None` when there's no filler cache available (graceful no-op).
fn spawn_filler_watchdog(
    filler_pcm: Option<Arc<Vec<f32>>>,
    sample_rate: u32,
    first_audio_emitted: Arc<AtomicBool>,
    out_tx: mpsc::Sender<Message>,
) -> Option<JoinHandle<()>> {
    let pcm = filler_pcm?;
    Some(tokio::spawn(async move {
        tokio::time::sleep(FILLER_TRIGGER).await;
        if first_audio_emitted.swap(true, Ordering::AcqRel) {
            debug!("voice.filler_skipped_kokoro_already_emitted");
            return;
        }
        metrics::counter!(CTR_FILLER_TRIGGER).increment(1);
        debug!(samples = pcm.len(), "voice.filler_emit");
        let Some(mut encoder) = make_opus_encoder(sample_rate) else {
            return;
        };
        let mut chunk_id: u64 = 0;
        for opus_packet in encode_pcm_with(&mut encoder, &pcm, sample_rate) {
            let framed = encode_tts_frame(chunk_id, FLAG_FILLER, &opus_packet);
            chunk_id = chunk_id.saturating_add(1);
            if out_tx.send(Message::Binary(framed.into())).await.is_err() {
                return;
            }
        }
    }))
}
