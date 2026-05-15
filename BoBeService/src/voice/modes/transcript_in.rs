//! Mode B dispatcher — text transcripts in, client owns ASR.
//!
//! Wired from `voice/control.rs::handle_control_text` on
//! `ClientMessage::TranscriptFinal`. Spawns a per-turn task that runs the
//! shared convergence body (`voice/run_text_turn::run_text_turn`), and
//! populates the session's `current_turn` slot so disconnect cleanup +
//! barge-in cancellation reuse the same plumbing.

use tokio::task::JoinHandle;
use tracing::{info, warn};

use crate::voice::context::VoiceContext;
use crate::voice::run_text_turn::run_text_turn;
use crate::voice::session::{SessionVoiceConfig, TurnInFlight, VoiceSession};

/// Entry point called from `control.rs` when a `TranscriptFinal` arrives.
/// Drops the message (with a warning) if there's no active session, the
/// session is muted, or a turn is already in flight (single-flight is
/// enforced structurally here in addition to the LLM-level guard inside
/// `run_text_turn`).
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
        chars = trimmed.len(),
        transcript = %trimmed,
        "voice.transcript_final"
    );

    let join: JoinHandle<()> = spawn_text_turn(text, turn_id.clone(), ctx, s);
    s.current_turn = Some(TurnInFlight {
        turn_id,
        join,
    });
}

/// Spawn the per-turn task. Disconnect cleanup + barge-in abort plumbing
/// reads `current_turn` regardless of how the turn started.
fn spawn_text_turn(
    text: String,
    turn_id: String,
    ctx: &VoiceContext,
    s: &VoiceSession,
) -> JoinHandle<()> {
    let task_ctx = ctx.clone();
    let voice_cfg = s.voice_cfg.clone();
    let language = s.language.clone();
    tokio::spawn(async move {
        run_mode_b_turn(text, turn_id, task_ctx, voice_cfg, language).await;
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
) {
    drop(language); // span field only
    let trimmed = text.trim();
    run_text_turn(trimmed, &turn_id, &ctx, voice_cfg).await;
}
