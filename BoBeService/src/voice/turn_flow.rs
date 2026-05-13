//! Per-turn lifecycle for voice WS sessions. Owns:
//!   - `spawn_turn` / `process_turn` — STT → LLM → TTS pipeline
//!   - `spawn_kokoro_task` / `spawn_filler_watchdog` — TTS encode tasks
//!   - `abort_active_turn` / `handle_barge_in` — interrupt paths
//!   - `reap_finished_turn` / `VoiceTurnFlag` — bookkeeping
//!
//! Extracted from `api/handlers/voice.rs` per the deep-decomp split plan
//! (step 5 of 6). The remaining handler is now just the WS loop +
//! `handle_control_text` dispatch.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use axum::extract::ws::Message;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::speech::TtsEngine;
use crate::speech::protocol::{FLAG_FILLER, FLAG_FIRST_OF_TURN, ServerMessage, VoicePhase, encode_tts_frame};
use crate::speech::vad::SpeechSegment;
use crate::voice::context::VoiceContext;
use crate::voice::engines::VoiceEngines;
use crate::voice::filler_library::FillerKind;
use crate::voice::opus::{encode_pcm_with, make_opus_encoder};
use crate::voice::protocol_helpers::{send_error, send_json, send_state};
use crate::voice::sentence_pipeline::SentencePipeline;
use crate::voice::session::{SessionVoiceConfig, TurnInFlight, VoiceSession};
use crate::voice::telemetry::{
    CTR_BARGE_IN_FALSE, CTR_BARGE_IN_SUCCESS, CTR_FILLER_TRIGGER, CTR_TURN_COMPLETE, CTR_TURN_ERROR,
    HIST_E2E_MS, HIST_SMART_TURN_MS, HIST_STT_MS,
};

/// Smart-turn pass-through threshold. < 0.7 → discard segment as a
/// mid-thought pause; ≥ 0.7 → user is done, proceed with the turn.
pub(crate) const TURN_COMPLETE_THRESHOLD: f32 = 0.7;

/// MinWords barge-in gate (C3). During an active turn, a barge-in is
/// honored only if the streaming-STT has accumulated at least this many
/// words of user speech. "uh-huh"/"yeah" backchannel tokens fall below
/// the threshold and get dropped as false barge-ins. Pipecat's default.
pub(crate) const MIN_WORDS_FOR_BARGE_IN: usize = 3;

/// Time-to-first-audio budget. If the Kokoro task hasn't pushed any audio
/// frames by this deadline (the LLM is slow or the first sentence is still
/// buffering), the filler watchdog emits the pre-rendered cached phrase.
/// Matches LiveKit/Pipecat/ElevenLabs production threshold.
pub(crate) const FILLER_TRIGGER: Duration = Duration::from_millis(800);

/// mpsc capacity between the LLM-observer (sync closure) and the Kokoro
/// task. Bursts during fast LLM token rates can push 3-5 sentences within
/// ~200ms; 16 gives comfortable headroom while still bounding memory.
pub(crate) const SENTENCE_CHANNEL_CAPACITY: usize = 16;

/// Reap a `current_turn` slot whose join handle has finished. Safe to call
/// from the WS loop; idempotent.
pub(crate) fn reap_finished_turn(session: &mut VoiceSession) {
    if let Some(turn) = session.current_turn.as_ref()
        && turn.join.is_finished()
    {
        session.current_turn = None;
    }
}

/// Spawn a per-turn task that owns the entire STT → LLM → TTS pipeline.
/// The returned `TurnInFlight` is stored on the session so a barge-in
/// can `.join.abort()` it. `ctx` is cloned into the task — cheap because
/// every field is `Arc` or value-snapshot.
pub(crate) fn spawn_turn(
    segment: SpeechSegment,
    ctx: &VoiceContext,
    voice_cfg: SessionVoiceConfig,
) -> TurnInFlight {
    let turn_id = format!("voice_{}", Uuid::new_v4().simple());
    let task_turn_id = turn_id.clone();
    let task_ctx = ctx.clone();
    let join = tokio::spawn(async move {
        process_turn(segment, &task_ctx, task_turn_id, voice_cfg).await;
    });
    TurnInFlight { turn_id, join }
}

/// RAII flag: voice_turn_active is set to true on construction and cleared on
/// drop. Ensures the BobeHooks signal goes back to false even if process_turn
/// errors out mid-flight.
struct VoiceTurnFlag(Arc<AtomicBool>);

impl Drop for VoiceTurnFlag {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[tracing::instrument(name = "voice.turn", skip_all, fields(turn_id = %turn_id, samples = segment.samples.len()))]
async fn process_turn(
    segment: SpeechSegment,
    ctx: &VoiceContext,
    turn_id: String,
    voice_cfg: SessionVoiceConfig,
) {
    let engines = &ctx.engines;
    let out_tx = &ctx.out_tx;
    let runtime_session = &ctx.runtime_session;
    let turn_start = Instant::now();

    // Semantic turn gate — discard if the user isn't really done yet.
    let smart_turn_start = Instant::now();
    match engines.smart_turn.probability_complete(&segment.samples) {
        Ok(p) if p < TURN_COMPLETE_THRESHOLD => {
            debug!(p, "voice.smart_turn_incomplete_skip");
            return;
        }
        Ok(p) => debug!(p, "voice.smart_turn_complete"),
        Err(e) => warn!(error = %e, "voice.smart_turn_failed_passthrough"),
    }
    metrics::histogram!(HIST_SMART_TURN_MS)
        .record(smart_turn_start.elapsed().as_secs_f64() * 1000.0);

    let guard = match runtime_session.try_begin_user_message() {
        Ok(g) => g,
        Err(reason) => {
            send_error(out_tx, "conflict", reason).await;
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

    send_state(out_tx, VoicePhase::Thinking, &turn_id).await;

    // STT — pull the final transcript out of the streaming Zipformer state
    // (which has been accumulating partials as frames arrived) and reset it
    // for the next turn. No more spawn_blocking on segment.samples.
    drop(segment.samples); // smart-turn already consumed it above
    let stt_start = Instant::now();
    let transcript = match engines.stt.commit_final() {
        Ok(text) => text,
        Err(e) => {
            error!(error = %e, "voice.stt_failed");
            send_error(out_tx, "stt_failed", &e.to_string()).await;
            send_state(out_tx, VoicePhase::Listening, &turn_id).await;
            drop(guard);
            return;
        }
    };

    let stt_elapsed_ms = stt_start.elapsed().as_millis() as u64;
    metrics::histogram!(HIST_STT_MS).record(stt_elapsed_ms as f64);
    let trimmed = transcript.trim();
    if trimmed.is_empty() {
        info!(stt_ms = stt_elapsed_ms, "voice.empty_transcript");
        metrics::counter!(CTR_TURN_ERROR).increment(1);
        send_error(out_tx, "empty_transcript", "I didn't catch that").await;
        send_state(out_tx, VoicePhase::Listening, &turn_id).await;
        drop(guard);
        return;
    }
    info!(
        stt_ms = stt_elapsed_ms,
        chars = trimmed.len(),
        transcript = %trimmed,
        "voice.transcript_final"
    );
    send_json(
        out_tx,
        &ServerMessage::TranscriptFinal {
            turn_id: turn_id.clone(),
            text: trimmed.to_string(),
        },
    )
    .await;

    // Spin up the per-turn TTS pipeline
    send_state(out_tx, VoicePhase::Speaking, &turn_id).await;
    let speaking_start = Instant::now();
    let first_audio_emitted = Arc::new(AtomicBool::new(false));
    let filler_task = spawn_filler_watchdog(
        engines.fillers.as_ref().and_then(|lib| lib.get(FillerKind::Thinking)),
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
        .handle_user_message_with_observer(trimmed, &turn_id, observer)
        .await;

    // Flush trailing sentence (LLM may end without trailing punctuation)
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

    // Cancel the filler watchdog if it hasn't fired yet (LLM was fast enough).
    if let Some(handle) = filler_task {
        handle.abort();
        drop(handle.await);
    }

    send_json(
        out_tx,
        &ServerMessage::TtsEnd {
            turn_id: turn_id.clone(),
        },
    )
    .await;
    send_state(out_tx, VoicePhase::Listening, &turn_id).await;
    drop(guard);

    let total_ms = turn_start.elapsed().as_millis() as u64;
    metrics::histogram!(HIST_E2E_MS).record(total_ms as f64);
    metrics::counter!(CTR_TURN_COMPLETE).increment(1);
    info!(
        stt_ms = stt_elapsed_ms,
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
        // One Opus encoder for the whole turn — reuses internal entropy
        // coder state across sentences for marginally better compression
        // than the per-call recreation we used to do.
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
                    // Claim the "first audio emitted" slot so the filler
                    // watchdog skips even if it's about to fire.
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
        // Atomic claim — if Kokoro already emitted, swap returns true and we exit.
        if first_audio_emitted.swap(true, Ordering::AcqRel) {
            debug!("voice.filler_skipped_kokoro_already_emitted");
            return;
        }
        metrics::counter!(CTR_FILLER_TRIGGER).increment(1);
        debug!(samples = pcm.len(), "voice.filler_emit");
        // Filler is one-shot per turn, so a fresh encoder is fine here.
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

/// Shared abort body for all "stop this turn" paths (RMS barge-in,
/// cancel phrase, explicit Abort/Reset control). Assumes the caller has
/// already authorized the abort — does NOT enforce MinWords.
pub(crate) async fn abort_active_turn(
    s: &mut VoiceSession,
    engines: &VoiceEngines,
    out_tx: &mpsc::Sender<Message>,
    keep_ms: u64,
    reason: &'static str,
) {
    let Some(turn) = s.current_turn.take() else {
        debug!(reason, "voice.abort_no_active_turn");
        return;
    };
    info!(
        turn_id = %turn.turn_id,
        keep_ms,
        reason,
        "voice.abort_active_turn"
    );
    let TurnInFlight { turn_id, join } = turn;
    join.abort();
    drop(join.await);
    send_json(
        out_tx,
        &ServerMessage::Truncate {
            turn_id: turn_id.clone(),
            keep_ms,
        },
    )
    .await;
    send_state(out_tx, VoicePhase::Listening, &turn_id).await;
    s.last_partial_text.clear();
    engines.stt.reset();
}

/// Client-detected RMS barge-in arriving over the WS. Gates on MinWords
/// to drop "uh-huh"/"yeah" backchannel, then delegates to
/// `abort_active_turn` for the actual cancellation.
pub(crate) async fn handle_barge_in(
    out_tx: &mpsc::Sender<Message>,
    session: &mut Option<VoiceSession>,
    engines: &VoiceEngines,
    played_ms: u64,
) {
    let Some(s) = session.as_mut() else { return };
    let word_count = s.last_partial_text.split_whitespace().count();
    if s.current_turn.is_some() && word_count < MIN_WORDS_FOR_BARGE_IN {
        metrics::counter!(CTR_BARGE_IN_FALSE).increment(1);
        debug!(
            words = word_count,
            partial = %s.last_partial_text,
            "voice.barge_in_dropped_min_words"
        );
        return;
    }
    if s.current_turn.is_none() {
        debug!("voice.barge_in_no_turn");
        return;
    }
    metrics::counter!(CTR_BARGE_IN_SUCCESS).increment(1);
    let keep_ms = played_ms.max(s.last_acked_played_ms);
    abort_active_turn(s, engines, out_tx, keep_ms, "barge_in").await;
}
