//! Spawns per-turn tasks for `ClientMessage::TranscriptFinal`; populates
//! `current_turn` so disconnect/barge-in cleanup find it.

use std::time::Instant;

use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tracing::{info, warn};

use crate::voice::context::VoiceContext;
use crate::voice::run_text_turn::run_text_turn;
use crate::voice::session::{SessionVoiceConfig, TurnInFlight, VoiceSession};

/// Drops with a warning when there's no session, the session is muted,
/// or a turn is already in flight (structural single-flight on top of
/// the LLM-level guard).
pub(crate) async fn dispatch_from_control(
    turn_id: String,
    text: String,
    ctx: &VoiceContext,
    session: &mut Option<VoiceSession>,
) {
    let Some(s) = session.as_mut() else {
        warn!(turn_id = %turn_id, "voice.transcript_final_no_session");
        return;
    };
    if s.muted {
        warn!(turn_id = %turn_id, "voice.transcript_final_dropped_muted");
        return;
    }
    if s.current_turn.is_some() {
        warn!(
            turn_id = %turn_id,
            "voice.transcript_final_during_active_turn_dropped"
        );
        return;
    }
    let trimmed = text.trim();
    if trimmed.is_empty() {
        warn!(turn_id = %turn_id, "voice.empty_transcript_final_dropped");
        return;
    }
    info!(
        turn_id = %turn_id,
        chars = trimmed.chars().count(),
        "voice.transcript_final"
    );
    // Partial evidence belongs to the user's interruption attempt, not to the
    // assistant response that starts now. Never let it authorize a later turn.
    s.last_partial_text.clear();

    let (playback_complete, playback_receiver) = if s.voice_cfg.client_tts {
        let (sender, receiver) = oneshot::channel();
        (Some(sender), Some(receiver))
    } else {
        (None, None)
    };
    let join: JoinHandle<()> = spawn_text_turn(text, turn_id.clone(), ctx, s, playback_receiver);
    s.current_turn = Some(TurnInFlight {
        turn_id,
        join,
        started_at: Instant::now(),
        playback_complete,
    });
}

/// Always signals `turn_completion_tx` on exit so the recv loop clears
/// `current_turn`; otherwise the single-flight gate would drop all
/// subsequent transcripts after this one finishes.
fn spawn_text_turn(
    text: String,
    turn_id: String,
    ctx: &VoiceContext,
    s: &VoiceSession,
    playback_complete: Option<oneshot::Receiver<()>>,
) -> JoinHandle<()> {
    let task_ctx = ctx.clone();
    let voice_cfg = s.voice_cfg.clone();
    let language = s.language.clone();
    let completion_tx = ctx.turn_completion_tx.clone();
    tokio::spawn(async move {
        run_mode_b_turn(
            text,
            turn_id,
            task_ctx,
            voice_cfg,
            language,
            playback_complete,
        )
        .await;
        // Recv dropped = WS torn down; ignore.
        let _ = completion_tx.send(()).await;
    })
}

#[tracing::instrument(
    name = "voice.turn",
    skip_all,
    fields(turn_id = %turn_id, source = "client_stt", language = %language)
)]
async fn run_mode_b_turn(
    text: String,
    turn_id: String,
    ctx: VoiceContext,
    voice_cfg: SessionVoiceConfig,
    language: String,
    playback_complete: Option<oneshot::Receiver<()>>,
) {
    drop(language); // span field only
    let trimmed = text.trim();
    run_text_turn(trimmed, &turn_id, &ctx, voice_cfg, playback_complete).await;
}
