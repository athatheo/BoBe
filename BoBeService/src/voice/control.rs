//! Voice WS control-message dispatch.
//!
//! Mode B only: client owns ASR. This module handles all inbound JSON
//! `ClientMessage` variants — there is no Binary path on the daemon.

use tracing::{debug, info, warn};

use crate::models::ids::new_turn_id;
use crate::speech::protocol::{ClientMessage, ControlAction, ServerMessage, VoicePhase};
use crate::voice::cancel_phrases::is_cancel_phrase;
use crate::voice::context::VoiceContext;
use crate::voice::protocol_helpers::{send_error, send_json, send_state};
use crate::voice::session::{SessionVoiceConfig, TTS_OUTPUT_SAMPLE_RATE, VoiceSession};
use crate::voice::telemetry::CTR_CANCEL_PHRASE;
use crate::voice::transcript_in;
use crate::voice::turn_flow::{abort_active_turn, handle_barge_in};

/// Default language when the client doesn't specify one at Hello.
const DEFAULT_LANGUAGE: &str = "en";

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
            playback_rate,
            voice_id,
            speed,
            language,
        }) => {
            let language = language.unwrap_or_else(|| DEFAULT_LANGUAGE.to_string());
            info!(
                session = %session_id,
                playback_rate,
                voice_id = ?voice_id,
                speed = ?speed,
                language = %language,
                "voice.hello"
            );
            if playback_rate != TTS_OUTPUT_SAMPLE_RATE {
                send_error(
                    out_tx,
                    "rate_mismatch",
                    &format!("expected playback_rate={TTS_OUTPUT_SAMPLE_RATE}"),
                )
                .await;
                return false;
            }
            let cfg = SessionVoiceConfig::new(voice_id, speed, voice_defaults);
            let voice_pack = cfg.voice_id.clone();
            let s = VoiceSession::new(session_id, cfg, language);
            let initial_turn = new_turn_id(false);
            // Rehello: a second Hello on the same WS replaces the session.
            // Abort the previous in-flight turn first — otherwise the
            // spawned task keeps running (JoinHandle::drop does NOT abort)
            // and orphans hold the Kokoro mutex + emit TTS frames the new
            // session can't reach for barge-in.
            if let Some(prev) = session.as_mut() {
                abort_active_turn(prev, ctx, 0, "rehello").await;
            }
            *session = Some(s);
            send_json(
                out_tx,
                &ServerMessage::HelloAck {
                    voice_pack,
                    playback_rate: TTS_OUTPUT_SAMPLE_RATE,
                },
            )
            .await;
            send_state(out_tx, VoicePhase::Listening, &initial_turn).await;
            true
        }
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
            // If no turn is in flight and the user hasn't muted, send
            // state(Listening) so the client opens the mic.
            if let Some(s) = session.as_mut()
                && s.current_turn.is_none()
                && !s.muted
            {
                let turn_id = new_turn_id(true);
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
        Ok(ClientMessage::TranscriptPartial { turn_id, text }) => {
            handle_transcript_partial(turn_id, text, ctx, session).await;
            true
        }
        Ok(ClientMessage::TranscriptFinal { turn_id, text }) => {
            transcript_in::dispatch_from_control(turn_id, text, ctx, session).await;
            true
        }
        Err(e) => {
            warn!(error = %e, "voice.invalid_json");
            send_error(out_tx, "invalid_json", &format!("{e}")).await;
            true
        }
    }
}

/// Update `session.last_partial_text` and check for cancel phrases that
/// should abort an active turn without invoking the LLM.
async fn handle_transcript_partial(
    turn_id: String,
    text: String,
    ctx: &VoiceContext,
    session: &mut Option<VoiceSession>,
) {
    let Some(s) = session.as_mut() else { return };
    if s.muted {
        debug!(turn_id = %turn_id, "voice.transcript_partial_dropped_muted");
        return;
    }
    s.last_partial_text = text;
    // Cancel-phrase detection on partial — bypasses the LLM entirely when
    // the user says "stop"/"nevermind"/etc. during TTS playback.
    if s.current_turn.is_some() && is_cancel_phrase(&s.last_partial_text) {
        info!(
            turn_id = %turn_id,
            partial = %s.last_partial_text,
            "voice.cancel_phrase_detected"
        );
        metrics::counter!(CTR_CANCEL_PHRASE).increment(1);
        let keep_ms = s.last_acked_played_ms;
        abort_active_turn(s, ctx, keep_ms, "cancel_phrase").await;
    }
}

pub(crate) async fn handle_control_action(
    action: ControlAction,
    ctx: &VoiceContext,
    session: &mut Option<VoiceSession>,
) {
    let out_tx = &ctx.out_tx;
    match action {
        ControlAction::Abort => {
            info!("voice.control.abort");
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
            handle_barge_in(ctx, session, 0).await;
            if let Some(s) = session.as_mut() {
                s.muted = false;
                send_state(out_tx, VoicePhase::Listening, &s.session_id).await;
            }
        }
    }
}
