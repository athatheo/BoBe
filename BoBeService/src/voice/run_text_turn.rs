//! Voice turn convergence helper. `transcript_in` calls this on a final
//! transcript; it owns single-flight admission, state emission
//! (Thinking → Speaking → Listening), filler + Kokoro task spawn, and
//! the LLM observer feeding `SentencePipeline`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tracing::{debug, error, warn};

use crate::models::ids::conversation_turn_id_from_wire_id;
use crate::speech::TtsEngine;
use crate::speech::protocol::{
    FLAG_FILLER, FLAG_FIRST_OF_TURN, ServerMessage, VoicePhase, encode_tts_frame,
};
use crate::util::atomic_flag_guard::AtomicFlagGuard;
use crate::voice::abort_on_drop::AbortOnDrop;
use crate::voice::context::VoiceContext;
use crate::voice::filler_library::FillerKind;
use crate::voice::opus::{encode_pcm_with, make_opus_encoder};
use crate::voice::output::VoiceOutput;
use crate::voice::protocol_helpers::{send_error, send_json, send_state};
use crate::voice::sentence_pipeline::SentencePipeline;
use crate::voice::session::SessionVoiceConfig;
use crate::voice::telemetry::{
    CTR_FILLER_TRIGGER, CTR_TURN_COMPLETE, HIST_E2E_MS, HIST_TTS_SYNTH_MS, HIST_TURN_TOTAL_MS,
};

/// Time-to-first-audio budget. If the Kokoro task hasn't pushed any audio
/// frames by this deadline (the LLM is slow or the first sentence is still
/// buffering), the filler watchdog emits the pre-rendered cached phrase.
/// Matches LiveKit/Pipecat/ElevenLabs production threshold.
pub(crate) const FILLER_TRIGGER: Duration = Duration::from_millis(800);

pub(crate) enum VoiceTurnAdmission {
    Acquire,
    AlreadyHeld,
}

/// The convergence body. Caller is responsible for pre-validating the
/// trimmed `text` (non-empty) and populating the session's `current_turn`
/// slot if barge-in must be honored.
pub(crate) async fn run_text_turn(
    text: &str,
    turn_id: &str,
    ctx: &VoiceContext,
    voice_cfg: SessionVoiceConfig,
    playback_complete: Option<oneshot::Receiver<()>>,
    admission: VoiceTurnAdmission,
) {
    let engines = &ctx.engines;
    let output = &ctx.output;
    let runtime_session = &ctx.runtime_session;
    let turn_start = Instant::now();
    let assistant_turn_id = match conversation_turn_id_from_wire_id(turn_id) {
        Ok(turn_id) => turn_id,
        Err(parse_error) => {
            error!(turn_id, error = %parse_error, "voice.invalid_turn_id");
            send_error(
                output,
                "invalid_turn_id",
                "The voice turn identifier is invalid",
            )
            .await;
            send_state(output, VoicePhase::Listening, turn_id).await;
            return;
        }
    };

    let guard = match admission {
        VoiceTurnAdmission::AlreadyHeld => None,
        VoiceTurnAdmission::Acquire => match runtime_session.try_begin_user_message() {
            Ok(guard) => Some(guard),
            Err(reason) => {
                send_error(output, "conflict", reason).await;
                // Roll the wire state back to Listening so the client UI
                // doesn't get stuck mid-turn (relevant since the caller has
                // pre-emitted Thinking before reaching here).
                send_state(output, VoicePhase::Listening, turn_id).await;
                return;
            }
        },
    };

    // Flag set AFTER single-flight admission — otherwise a microsecond
    // window between flag-set and try_begin_user_message failure could let
    // another worker class's UserPromptSubmitted hook see voice_mode=true
    // and inject the voice-tone hint into a non-voice turn. RAII guard
    // clears on drop including panic-unwind.
    ctx.voice_turn_active.store(true, Ordering::Release);
    let _voice_flag = AtomicFlagGuard::new(Arc::clone(&ctx.voice_turn_active));
    let _sink_guard = ctx.voice_sink.install(output.clone()).await;

    send_state(output, VoicePhase::Thinking, turn_id).await;
    send_json(
        output,
        &ServerMessage::TranscriptFinal {
            turn_id: turn_id.to_string(),
            text: text.to_string(),
        },
    )
    .await;

    send_state(output, VoicePhase::Speaking, turn_id).await;
    let speaking_start = Instant::now();
    let first_audio_emitted = Arc::new(AtomicBool::new(false));
    // Wrap children in AbortOnDrop so a parent abort (barge-in / disconnect
    // / cancel-phrase / rehello) cascades. Without this the kokoro task
    // keeps holding `Mutex<OfflineTts>` for 1-3s after the parent unwinds,
    // blocking the next turn's first synth.
    let tts = if voice_cfg.client_tts {
        None
    } else {
        let Some(tts) = engines.tts.as_ref().map(Arc::clone) else {
            send_error(output, "engines_unavailable", "Kokoro is not installed").await;
            send_state(output, VoicePhase::Listening, turn_id).await;
            return;
        };
        Some(tts)
    };
    let filler_task = tts.as_ref().and_then(|tts| {
        spawn_filler_watchdog(
            engines
                .fillers
                .as_ref()
                .and_then(|lib| lib.get(FillerKind::Thinking)),
            tts.sample_rate(),
            Arc::clone(&first_audio_emitted),
            output.clone(),
            turn_start,
        )
        .map(AbortOnDrop::new)
    });
    let (sentence_tx, sentence_rx) = mpsc::channel::<String>(16);
    let voice_output_task = if voice_cfg.client_tts {
        AbortOnDrop::new(spawn_client_tts_text_task(
            turn_id.to_string(),
            sentence_rx,
            output.clone(),
        ))
    } else {
        let Some(tts) = tts else {
            return;
        };
        AbortOnDrop::new(spawn_kokoro_task(
            tts,
            turn_id.to_string(),
            sentence_rx,
            output.clone(),
            Arc::clone(&first_audio_emitted),
            voice_cfg.clone(),
            ctx.send_server_tts_text,
            turn_start,
        ))
    };

    let pipeline = Arc::new(tokio::sync::Mutex::new(SentencePipeline::new(turn_start)));
    let pipeline_for_obs = Arc::clone(&pipeline);
    let sentence_tx_for_obs = sentence_tx.clone();
    let observer = move |delta: String| {
        let pipeline = Arc::clone(&pipeline_for_obs);
        let sentence_tx = sentence_tx_for_obs.clone();
        async move {
            let sentences = pipeline.lock().await.feed(&delta);
            for sentence in sentences {
                if sentence_tx.send(sentence).await.is_err() {
                    return;
                }
            }
        }
    };

    let turn_succeeded = runtime_session
        .handle_user_message_with_observer(text, turn_id, assistant_turn_id, ctx.delivery, observer)
        .await;
    if !turn_succeeded {
        drop(sentence_tx);
        abort_and_settle(voice_output_task, "voice output").await;
        if let Some(filler) = filler_task {
            abort_and_settle(filler, "filler").await;
        }
        send_error(output, "turn_failed", "The assistant turn did not complete").await;
        send_state(output, VoicePhase::Listening, turn_id).await;
        return;
    }

    let remaining = pipeline.lock().await.flush();
    for sentence in remaining {
        if sentence_tx.send(sentence).await.is_err() {
            break;
        }
    }
    drop(pipeline);
    drop(sentence_tx);
    // Strip the AbortOnDrop wrappers on the natural-completion path so
    // .await + .abort() run instead of the wrapper's Drop firing first.
    let voice_output_handle = voice_output_task.into_inner();
    let output_produced = match voice_output_handle.await {
        Ok(produced) => produced,
        Err(e) if e.is_panic() => {
            error!(error = %e, "voice.output_task_panic");
            false
        }
        Err(e) if e.is_cancelled() => {
            debug!("voice.output_task_cancelled");
            false
        }
        Err(e) => {
            warn!(error = %e, "voice.output_task_join_failed");
            false
        }
    };

    if let Some(filler) = filler_task {
        abort_and_settle(filler, "filler").await;
    }
    if !output_produced {
        send_error(
            output,
            "tts_failed",
            "Response audio could not be completed",
        )
        .await;
        send_state(output, VoicePhase::Listening, turn_id).await;
        return;
    }

    send_json(
        output,
        &ServerMessage::TtsEnd {
            turn_id: turn_id.to_string(),
        },
    )
    .await;
    if let Some(receiver) = playback_complete {
        match tokio::time::timeout(Duration::from_mins(10), receiver).await {
            Ok(Ok(())) => {}
            Ok(Err(_)) => debug!("voice.client_tts_completion_sender_dropped"),
            Err(_) => warn!("voice.client_tts_completion_timeout"),
        }
    }
    send_state(output, VoicePhase::Listening, turn_id).await;
    drop(guard);

    let total_ms = turn_start.elapsed().as_millis() as u64;
    metrics::histogram!(HIST_TURN_TOTAL_MS).record(total_ms as f64);
    metrics::counter!(CTR_TURN_COMPLETE).increment(1);
    tracing::info!(
        speaking_ms = speaking_start.elapsed().as_millis() as u64,
        total_ms,
        "voice.turn_complete"
    );
}

async fn abort_and_settle<T>(task: AbortOnDrop<T>, task_name: &'static str) {
    let handle = task.into_inner();
    handle.abort();
    match handle.await {
        Err(error) if error.is_cancelled() => {}
        Err(error) => warn!(%error, task_name, "voice.child_task_join_failed"),
        Ok(_) => debug!(task_name, "voice.child_task_completed_before_abort"),
    }
}

fn spawn_client_tts_text_task(
    turn_id: String,
    mut sentence_rx: mpsc::Receiver<String>,
    output: VoiceOutput,
) -> tokio::task::JoinHandle<bool> {
    tokio::spawn(async move {
        let mut sequence = 0_u64;
        let mut produced = false;
        while let Some(text) = sentence_rx.recv().await {
            produced = true;
            send_json(
                &output,
                &ServerMessage::TtsText {
                    turn_id: turn_id.clone(),
                    sequence,
                    text,
                },
            )
            .await;
            sequence = sequence.saturating_add(1);
        }
        produced
    })
}

fn spawn_kokoro_task(
    tts: Arc<dyn TtsEngine>,
    turn_id: String,
    mut sentence_rx: mpsc::Receiver<String>,
    output: VoiceOutput,
    first_audio_emitted: Arc<AtomicBool>,
    voice_cfg: SessionVoiceConfig,
    send_tts_text: bool,
    turn_start: Instant,
) -> tokio::task::JoinHandle<bool> {
    let sample_rate = tts.sample_rate();
    tokio::spawn(async move {
        let Some(mut encoder) = make_opus_encoder(sample_rate) else {
            return false;
        };
        let mut chunk_id: u64 = 0;
        let mut sentence_sequence: u64 = 0;
        let mut first_chunk_pending = true;
        let mut produced = false;
        while let Some(sentence) = sentence_rx.recv().await {
            let tts_clone = Arc::clone(&tts);
            let sentence_owned = sentence.clone();
            let voice_id = voice_cfg.voice_id.clone();
            let speed = voice_cfg.speed;
            let synth_start = Instant::now();
            let synth = tokio::task::spawn_blocking(move || {
                tts_clone.synthesize(&sentence_owned, &voice_id, speed)
            })
            .await;
            let pcm = match synth {
                Ok(Ok(pcm)) => pcm,
                Ok(Err(e)) => {
                    warn!(error = %e, "voice.kokoro_synth_failed");
                    return false;
                }
                Err(e) => {
                    warn!(error = %e, "voice.kokoro_join_failed");
                    return false;
                }
            };
            metrics::histogram!(HIST_TTS_SYNTH_MS)
                .record(synth_start.elapsed().as_secs_f64() * 1_000.0);
            if send_tts_text {
                send_json(
                    &output,
                    &ServerMessage::TtsText {
                        turn_id: turn_id.clone(),
                        sequence: sentence_sequence,
                        text: sentence,
                    },
                )
                .await;
                sentence_sequence = sentence_sequence.saturating_add(1);
            }
            for opus_packet in encode_pcm_with(&mut encoder, &pcm, sample_rate) {
                let mut flags = 0_u8;
                if first_chunk_pending {
                    flags |= FLAG_FIRST_OF_TURN;
                    first_chunk_pending = false;
                    first_audio_emitted.swap(true, Ordering::AcqRel);
                    metrics::histogram!(HIST_E2E_MS)
                        .record(turn_start.elapsed().as_secs_f64() * 1_000.0);
                }
                let framed = encode_tts_frame(chunk_id, flags, &opus_packet);
                chunk_id = chunk_id.saturating_add(1);
                if !output.audio(framed).await {
                    return produced;
                }
                produced = true;
            }
        }
        produced
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
    output: VoiceOutput,
    turn_start: Instant,
) -> Option<JoinHandle<()>> {
    let pcm = filler_pcm?;
    Some(tokio::spawn(async move {
        tokio::time::sleep(FILLER_TRIGGER).await;
        if first_audio_emitted.swap(true, Ordering::AcqRel) {
            debug!("voice.filler_skipped_kokoro_already_emitted");
            return;
        }
        metrics::histogram!(HIST_E2E_MS).record(turn_start.elapsed().as_secs_f64() * 1_000.0);
        metrics::counter!(CTR_FILLER_TRIGGER).increment(1);
        debug!(samples = pcm.len(), "voice.filler_emit");
        let Some(mut encoder) = make_opus_encoder(sample_rate) else {
            return;
        };
        let mut chunk_id: u64 = 0;
        for opus_packet in encode_pcm_with(&mut encoder, &pcm, sample_rate) {
            let framed = encode_tts_frame(chunk_id, FLAG_FILLER, &opus_packet);
            chunk_id = chunk_id.saturating_add(1);
            if !output.audio(framed).await {
                return;
            }
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::output::VoiceOutputFrame;

    #[tokio::test]
    async fn settled_child_cannot_emit_audio_after_terminal_state() {
        let (output, mut receiver) = VoiceOutput::channel(4);
        let child_output = output.clone();
        let child = AbortOnDrop::new(tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            child_output.audio(vec![1, 2, 3]).await
        }));

        abort_and_settle(child, "test output").await;
        send_state(&output, VoicePhase::Listening, "turn_test").await;
        drop(output);

        let mut frames = Vec::new();
        while let Some(frame) = receiver.recv().await {
            frames.push(frame);
        }
        assert_eq!(frames.len(), 1);
        std::assert_matches!(frames.first(), Some(VoiceOutputFrame::Control(_)));
    }
}
