//! Per-turn lifecycle bookkeeping shared across voice paths.
//!
//! Mode B only — the actual convergence body lives in
//! `voice/run_text_turn`. This module owns the barge-in cascade, the
//! `current_turn` reaper, and the MinWords gate.

use tracing::{debug, info};

use crate::speech::protocol::{ServerMessage, VoicePhase};
use crate::voice::context::VoiceContext;
use crate::voice::protocol_helpers::{send_json, send_state};
use crate::voice::session::{TurnInFlight, VoiceSession};
use crate::voice::telemetry::{CTR_BARGE_IN_FALSE, CTR_BARGE_IN_SUCCESS};

/// MinWords barge-in gate. During an active turn, a barge-in is honored
/// only if the streaming-STT has accumulated at least this many words of
/// user speech. "uh-huh"/"yeah" backchannel tokens fall below the threshold
/// and get dropped as false barge-ins.
pub(crate) const MIN_WORDS_FOR_BARGE_IN: usize = 3;

pub(crate) fn has_barge_in_evidence(partial: &str) -> bool {
    partial.split_whitespace().count() >= MIN_WORDS_FOR_BARGE_IN
}

/// Shared abort body for all "stop this turn" paths (RMS barge-in,
/// cancel phrase, explicit Abort/Reset control). Assumes the caller has
/// already authorized the abort — does NOT enforce MinWords.
pub(crate) async fn abort_active_turn(
    s: &mut VoiceSession,
    ctx: &VoiceContext,
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
    let TurnInFlight { turn_id, join, .. } = turn;
    join.abort();
    drop(join.await);
    send_json(
        &ctx.out_tx,
        &ServerMessage::Truncate {
            turn_id: turn_id.clone(),
            keep_ms,
        },
    )
    .await;
    send_state(&ctx.out_tx, VoicePhase::Listening, &turn_id).await;
    s.last_partial_text.clear();
}

/// Client-detected RMS barge-in arriving over the WS. Gates on MinWords
/// to drop "uh-huh"/"yeah" backchannel, then delegates to
/// `abort_active_turn` for the actual cancellation.
pub(crate) async fn handle_barge_in(
    ctx: &VoiceContext,
    session: &mut Option<VoiceSession>,
    played_ms: u64,
) {
    let Some(s) = session.as_mut() else { return };
    let word_count = s.last_partial_text.split_whitespace().count();
    if s.current_turn.is_some() && !has_barge_in_evidence(&s.last_partial_text) {
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
    abort_active_turn(s, ctx, keep_ms, "barge_in").await;
}

#[cfg(test)]
mod tests {
    use super::has_barge_in_evidence;

    #[test]
    fn acoustic_barge_in_requires_three_words() {
        assert!(!has_barge_in_evidence(""));
        assert!(!has_barge_in_evidence("uh huh"));
        assert!(has_barge_in_evidence("please stop now"));
    }
}
