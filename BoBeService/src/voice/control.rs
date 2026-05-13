//! Voice WS control-message dispatch.
//!
//! Owns `handle_control_text` (top-level JSON parse + match on
//! `ClientMessage` variants) and `handle_control_action` (Mute/Unmute/
//! Reset/Abort dispatch). Extracted from `api/handlers/voice.rs` per the
//! deep-decomp split plan; the WS handler now just routes Text frames here
//! and Binary frames into the VAD/STT pipeline.

use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::speech::protocol::{ClientMessage, ControlAction, VoicePhase};
use crate::voice::context::VoiceContext;
use crate::voice::protocol_helpers::{send_error, send_state};
use crate::voice::session::{
    OPUS_INPUT_SAMPLE_RATE, SessionVoiceConfig, TTS_OUTPUT_SAMPLE_RATE, VoiceSession,
};
use crate::voice::turn_flow::handle_barge_in;

/// Returns `false` to terminate the connection (after handshake errors).
pub(crate) async fn handle_control_text(
    text: &str,
    ctx: &VoiceContext,
    session: &mut Option<VoiceSession>,
) -> bool {
    let out_tx = &ctx.out_tx;
    let voice_defaults = &ctx.voice_defaults;
    let parsed: Result<ClientMessage, _> = serde_json::from_str(text);
    match parsed {
        Ok(ClientMessage::Hello {
            session_id,
            capture_rate,
            playback_rate,
            codec,
            voice_id,
            speed,
        }) => {
            info!(
                session = %session_id,
                capture_rate,
                playback_rate,
                codec,
                voice_id = ?voice_id,
                speed = ?speed,
                "voice.hello"
            );
            if capture_rate != OPUS_INPUT_SAMPLE_RATE {
                send_error(
                    out_tx,
                    "rate_mismatch",
                    &format!("expected capture_rate={OPUS_INPUT_SAMPLE_RATE}"),
                )
                .await;
                return false;
            }
            if playback_rate != TTS_OUTPUT_SAMPLE_RATE {
                send_error(
                    out_tx,
                    "rate_mismatch",
                    &format!("expected playback_rate={TTS_OUTPUT_SAMPLE_RATE}"),
                )
                .await;
                return false;
            }
            if codec != "opus" {
                send_error(out_tx, "unsupported_codec", &format!("expected opus, got {codec}"))
                    .await;
                return false;
            }
            let cfg = SessionVoiceConfig::new(voice_id, speed, voice_defaults);
            match VoiceSession::new(session_id, cfg) {
                Ok(s) => {
                    let initial_turn = format!("voice_{}", Uuid::new_v4().simple());
                    *session = Some(s);
                    send_state(out_tx, VoicePhase::Listening, &initial_turn).await;
                    true
                }
                Err(e) => {
                    error!(error = %e, "voice.session_init_failed");
                    send_error(out_tx, "session_init_failed", e).await;
                    false
                }
            }
        }
        Ok(ClientMessage::VadHint { .. }) => true,
        Ok(ClientMessage::BargeIn {
            ts_ms,
            playback_ms_played,
        }) => {
            info!(ts_ms, playback_ms_played, "voice.barge_in_received");
            handle_barge_in(ctx, session, playback_ms_played).await;
            true
        }
        Ok(ClientMessage::Wake {
            phrase,
            score,
            ts_ms,
        }) => {
            info!(phrase = %phrase, score, ts_ms, "voice.wake_received");
            // Wake hookup (E5): if no turn is in flight and the user hasn't
            // muted, send state(Listening) so the client opens the mic and
            // the daemon's Silero starts processing inbound audio.
            // Rate limit: do nothing if we're not in a steady listening
            // state — a mid-turn wake is the user changing their mind, and
            // the existing barge-in path handles that.
            if let Some(s) = session.as_mut()
                && s.current_turn.is_none()
                && !s.muted
            {
                let turn_id = format!("voice_wake_{}", Uuid::new_v4().simple());
                send_state(out_tx, VoicePhase::Listening, &turn_id).await;
            }
            true
        }
        Ok(ClientMessage::PlaybackAck {
            chunk_id,
            played_ms,
        }) => {
            if let Some(s) = session.as_mut() {
                // Monotonic — never roll back even if a stale ack arrives.
                if played_ms > s.last_acked_played_ms {
                    s.last_acked_played_ms = played_ms;
                }
            }
            debug!(chunk_id, played_ms, "voice.playback_ack");
            true
        }
        Ok(ClientMessage::Control { action }) => {
            handle_control_action(action, ctx, session).await;
            true
        }
        Err(e) => {
            warn!(error = %e, "voice.invalid_json");
            send_error(out_tx, "invalid_json", &format!("{e}")).await;
            true
        }
    }
}

pub(crate) async fn handle_control_action(
    action: ControlAction,
    ctx: &VoiceContext,
    session: &mut Option<VoiceSession>,
) {
    let out_tx = &ctx.out_tx;
    let engines = &ctx.engines;
    match action {
        ControlAction::Abort => {
            info!("voice.control.abort");
            // Treat explicit abort like a barge-in with played_ms=0.
            handle_barge_in(ctx, session, 0).await;
            if let Some(s) = session.as_ref() {
                send_state(out_tx, VoicePhase::Idle, &s.session_id).await;
            }
        }
        ControlAction::Mute => {
            if let Some(s) = session.as_mut() {
                s.muted = true;
                info!(session = %s.session_id, "voice.control.mute");
            }
        }
        ControlAction::Unmute => {
            if let Some(s) = session.as_mut() {
                s.muted = false;
                info!(session = %s.session_id, "voice.control.unmute");
            }
        }
        ControlAction::Reset => {
            info!("voice.control.reset");
            // Abort any in-flight turn (same path as Abort with played_ms=0),
            // clear VAD buffers, and unmute so the next utterance is captured.
            handle_barge_in(ctx, session, 0).await;
            engines.vad.reset();
            if let Some(s) = session.as_mut() {
                s.muted = false;
                send_state(out_tx, VoicePhase::Listening, &s.session_id).await;
            }
        }
    }
}
