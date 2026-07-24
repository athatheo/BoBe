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
    let output = &ctx.output;
    let voice_defaults = &ctx.voice_defaults;
    let parsed: Result<ClientMessage, _> = serde_json::from_str(text);
    match parsed {
        Ok(ClientMessage::Hello {
            session_id,
            playback_rate,
            voice_id,
            speed,
            language,
            tts_backend,
        }) => {
            let language = language.unwrap_or_else(|| DEFAULT_LANGUAGE.to_string());
            info!(
                session = %session_id,
                playback_rate,
                voice_id = ?voice_id,
                speed = ?speed,
                language = %language,
                tts_backend = ?tts_backend,
                "voice.hello"
            );
            if playback_rate != TTS_OUTPUT_SAMPLE_RATE {
                send_error(
                    output,
                    "rate_mismatch",
                    &format!("expected playback_rate={TTS_OUTPUT_SAMPLE_RATE}"),
                )
                .await;
                return false;
            }
            let cfg =
                SessionVoiceConfig::new(voice_id, speed, tts_backend.as_deref(), voice_defaults);
            if !cfg.client_tts && !ctx.engines.supports_server_tts() {
                send_error(
                    output,
                    "engines_unavailable",
                    "Kokoro is not installed; select client Supertonic or install Kokoro",
                )
                .await;
                return false;
            }
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
                output,
                &ServerMessage::HelloAck {
                    voice_pack,
                    playback_rate: TTS_OUTPUT_SAMPLE_RATE,
                },
            )
            .await;
            send_state(output, VoicePhase::Listening, &initial_turn).await;
            true
        }
        Ok(ClientMessage::TtsPlaybackStarted {
            turn_id,
            synthesis_ms,
        }) => {
            metrics::histogram!(crate::voice::telemetry::HIST_TTS_SYNTH_MS)
                .record(synthesis_ms as f64);
            if let Some(turn) = session
                .as_ref()
                .and_then(|session| session.current_turn.as_ref())
                .filter(|turn| turn.turn_id == turn_id)
            {
                metrics::histogram!(crate::voice::telemetry::HIST_E2E_MS)
                    .record(turn.started_at.elapsed().as_secs_f64() * 1_000.0);
            }
            true
        }
        Ok(ClientMessage::TtsPlaybackComplete { turn_id }) => {
            if let Some(sender) = session
                .as_mut()
                .and_then(|session| session.current_turn.as_mut())
                .filter(|turn| turn.turn_id == turn_id)
                .and_then(|turn| turn.playback_complete.take())
                && sender.send(()).is_err()
            {
                debug!("voice.client_tts_completion_receiver_dropped");
            }
            true
        }
        Ok(ClientMessage::BargeIn {
            ts_ms,
            playback_ms_played,
            partial_text,
        }) => {
            info!(ts_ms, playback_ms_played, "voice.barge_in_received");
            if let (Some(s), Some(evidence)) = (session.as_mut(), partial_text) {
                s.last_partial_text = evidence;
            }
            handle_barge_in(ctx, session, playback_ms_played).await;
            true
        }
        Ok(ClientMessage::Wake {
            phrase,
            score,
            ts_ms,
        }) => {
            info!(
                phrase_chars = phrase.chars().count(),
                score, ts_ms, "voice.wake_received"
            );
            // If no turn is in flight and the user hasn't muted, send
            // state(Listening) so the client opens the mic.
            if let Some(s) = session.as_mut()
                && s.current_turn.is_none()
                && !s.muted
            {
                let turn_id = new_turn_id(true);
                send_state(output, VoicePhase::Listening, &turn_id).await;
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
            send_error(output, "invalid_json", &format!("{e}")).await;
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
            partial_chars = s.last_partial_text.chars().count(),
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
    let output = &ctx.output;
    match action {
        ControlAction::Abort => {
            info!("voice.control.abort");
            if let Some(s) = session.as_mut() {
                abort_active_turn(s, ctx, 0, "control_abort").await;
                s.last_partial_text.clear();
                send_state(output, VoicePhase::Idle, &s.session_id).await;
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
            if let Some(s) = session.as_mut() {
                abort_active_turn(s, ctx, 0, "control_reset").await;
                s.last_partial_text.clear();
                s.muted = false;
                send_state(output, VoicePhase::Listening, &s.session_id).await;
            }
        }
    }
}
