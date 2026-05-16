//! WS wire protocol for `/voice/stream`.
//!
//! Mode B only: client owns ASR (FluidAudio Parakeet EOU / Qwen3-ASR via the
//! Swift `Voice/` module). Daemon owns LLM + TTS. Wire carries transcripts
//! and control in JSON; daemon-to-client TTS audio in Opus binary frames.
//! Client never sends audio.

use serde::{Deserialize, Serialize};

/// Control actions the client may request mid-session.
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ControlAction {
    /// Cancel the in-flight turn (user-initiated barge-in or explicit abort).
    Abort,
    /// Pause input — client should stop forwarding ASR transcripts until Unmute.
    Mute,
    Unmute,
    /// Reset session state (clear running turn; conversation context preserved).
    Reset,
}

/// Daemon-side authoritative turn phase. Client mirrors for UI only.
/// Client-driven states (`Connecting`, `Cancelling`, `Failed`) live in the
/// Swift `VoicePipeline.State` enum and never cross the wire.
///
/// **Wire contract:** match `BoBeMacUI/BoBe/Voice/VoiceProtocol.swift::
/// VoicePhaseWire` variant set. Adding a variant here without the Swift
/// counterpart makes decode fail on the client; the converse silently
/// drops unknown phases.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum VoicePhase {
    Idle,
    Listening,
    Thinking,
    Speaking,
}

/// **Wire contract:** `type` tag values (`hello`, `barge_in`, `wake`,
/// `playback_ack`, `control`, `transcript_partial`, `transcript_final`)
/// must match `BoBeMacUI/BoBe/Voice/VoiceProtocol.swift::ClientVoiceMessage`
/// encoder/decoder. Field names inside each variant mirror the Swift
/// `CodingKeys` switch — if you add/rename one, update both files.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(
    dead_code,
    reason = "BargeIn/Wake/PlaybackAck fields are wire-protocol contract"
)]
pub(crate) enum ClientMessage {
    /// Session handshake; sent once after WS connect. `language` (BCP-47)
    /// drives daemon-side telemetry/tracing; the client decides which local
    /// ASR engine to use. `voice_id`/`speed` override the daemon's Kokoro
    /// persona defaults for this WS only.
    Hello {
        session_id: String,
        playback_rate: u32,
        #[serde(default)]
        voice_id: Option<String>,
        #[serde(default)]
        speed: Option<f32>,
        #[serde(default)]
        language: Option<String>,
    },
    /// Client detected speech during BoBe TTS playback — candidate barge-in.
    /// Daemon decides whether to honour after the min-words gate.
    BargeIn { ts_ms: u64, playback_ms_played: u64 },
    /// Wake-word fired locally. Daemon may auto-open mic if not yet active.
    Wake {
        phrase: String,
        score: f32,
        ts_ms: u64,
    },
    /// Reports how much of TTS chunk_id has actually played, for truncation math.
    PlaybackAck { chunk_id: u64, played_ms: u64 },
    /// User-initiated control.
    Control { action: ControlAction },
    /// Client's streaming ASR emitted a partial transcript. Daemon stores it
    /// on `session.last_partial_text` for the cancel-phrase regex and the
    /// MinWords barge-in gate.
    TranscriptPartial { turn_id: String, text: String },
    /// Client's streaming ASR finalized this turn's text. Daemon admits the
    /// turn (single-flight) and hands off to the convergence pipeline.
    TranscriptFinal { turn_id: String, text: String },
}

/// **Wire contract:** `type` tag values (`hello_ack`, `state`, `tts_end`,
/// `truncate`, `transcript_final`, `error`) must match
/// `BoBeMacUI/BoBe/Voice/VoiceProtocol.swift::ServerVoiceMessage` decoder.
/// Note Binary TTS frames bypass this enum — see `encode_tts_frame` below.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ServerMessage {
    /// Capability ack sent once, immediately after a valid `Hello`, before
    /// any `State`. Confirms the resolved Kokoro persona and playback rate.
    HelloAck {
        voice_pack: String,
        playback_rate: u32,
    },
    /// Authoritative phase transition. Sent on every state change.
    State { phase: VoicePhase, turn_id: String },
    /// Echo of the client-finalized transcript for chat persistence + UI.
    TranscriptFinal { turn_id: String, text: String },
    /// All sentences flushed for this turn — client may leave Speaking.
    TtsEnd { turn_id: String },
    /// Post-barge-in: client drops queued audio beyond `keep_ms`.
    Truncate { turn_id: String, keep_ms: u64 },
    /// Non-fatal error message.
    Error { code: String, message: String },
}

/// Binary frame layout for `tts.chunk` and `filler.chunk`:
/// `[8 bytes BE u64 chunk_id][1 byte flags][N bytes Opus packet]`.
///
/// flags bit 0: 1 = filler (preemptible by real reply)
/// flags bit 1: 1 = first chunk of turn
pub(crate) const TTS_FRAME_HEADER_LEN: usize = 9;
pub(crate) const FLAG_FILLER: u8 = 0b0000_0001;
pub(crate) const FLAG_FIRST_OF_TURN: u8 = 0b0000_0010;

pub(crate) fn encode_tts_frame(chunk_id: u64, flags: u8, opus: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(TTS_FRAME_HEADER_LEN + opus.len());
    buf.extend_from_slice(&chunk_id.to_be_bytes());
    buf.push(flags);
    buf.extend_from_slice(opus);
    buf
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
    use super::*;
    use serde_json::json;

    #[test]
    fn hello_deserialize_minimal() {
        let raw = r#"{"type":"hello","session_id":"abc","playback_rate":24000}"#;
        let parsed: ClientMessage = serde_json::from_str(raw).unwrap();
        match parsed {
            ClientMessage::Hello {
                session_id,
                playback_rate,
                voice_id,
                speed,
                language,
            } => {
                assert_eq!(session_id, "abc");
                assert_eq!(playback_rate, 24_000);
                assert_eq!(voice_id, None);
                assert_eq!(speed, None);
                assert_eq!(language, None);
            }
            other => panic!("expected Hello, got {other:?}"),
        }
    }

    #[test]
    fn hello_deserialize_full() {
        let raw = r#"{
            "type":"hello",
            "session_id":"abc",
            "playback_rate":24000,
            "voice_id":"am_michael",
            "speed":1.2,
            "language":"zh"
        }"#;
        let parsed: ClientMessage = serde_json::from_str(raw).unwrap();
        match parsed {
            ClientMessage::Hello {
                voice_id,
                speed,
                language,
                ..
            } => {
                assert_eq!(voice_id, Some("am_michael".into()));
                assert_eq!(speed, Some(1.2));
                assert_eq!(language.as_deref(), Some("zh"));
            }
            other => panic!("expected Hello, got {other:?}"),
        }
    }

    #[test]
    fn transcript_partial_deserialize() {
        let raw = r#"{"type":"transcript_partial","turn_id":"voice_t1","text":"hello wor"}"#;
        let parsed: ClientMessage = serde_json::from_str(raw).unwrap();
        match parsed {
            ClientMessage::TranscriptPartial { turn_id, text } => {
                assert_eq!(turn_id, "voice_t1");
                assert_eq!(text, "hello wor");
            }
            other => panic!("expected TranscriptPartial, got {other:?}"),
        }
    }

    #[test]
    fn transcript_final_deserialize() {
        let raw = r#"{"type":"transcript_final","turn_id":"voice_t1","text":"hello world"}"#;
        let parsed: ClientMessage = serde_json::from_str(raw).unwrap();
        match parsed {
            ClientMessage::TranscriptFinal { turn_id, text } => {
                assert_eq!(turn_id, "voice_t1");
                assert_eq!(text, "hello world");
            }
            other => panic!("expected TranscriptFinal, got {other:?}"),
        }
    }

    #[test]
    fn barge_in_deserialize() {
        let raw = r#"{"type":"barge_in","ts_ms":1000,"playback_ms_played":420}"#;
        let parsed: ClientMessage = serde_json::from_str(raw).unwrap();
        match parsed {
            ClientMessage::BargeIn {
                ts_ms,
                playback_ms_played,
            } => {
                assert_eq!(ts_ms, 1_000);
                assert_eq!(playback_ms_played, 420);
            }
            other => panic!("expected BargeIn, got {other:?}"),
        }
    }

    #[test]
    fn control_abort_deserialize() {
        let raw = r#"{"type":"control","action":"abort"}"#;
        let parsed: ClientMessage = serde_json::from_str(raw).unwrap();
        assert!(matches!(
            parsed,
            ClientMessage::Control {
                action: ControlAction::Abort
            }
        ));
    }

    #[test]
    fn hello_ack_serialize() {
        let msg = ServerMessage::HelloAck {
            voice_pack: "af_bella".into(),
            playback_rate: 24_000,
        };
        let v = serde_json::to_value(&msg).unwrap();
        assert_eq!(
            v,
            json!({
                "type":"hello_ack",
                "voice_pack":"af_bella",
                "playback_rate":24000
            })
        );
    }

    #[test]
    fn state_serialize_uses_snake_case() {
        let msg = ServerMessage::State {
            phase: VoicePhase::Speaking,
            turn_id: "voice_xyz".into(),
        };
        let v = serde_json::to_value(&msg).unwrap();
        assert_eq!(
            v,
            json!({"type":"state","phase":"speaking","turn_id":"voice_xyz"})
        );
    }

    #[test]
    fn transcript_final_serialize() {
        let msg = ServerMessage::TranscriptFinal {
            turn_id: "voice_xyz".into(),
            text: "hello world".into(),
        };
        let v = serde_json::to_value(&msg).unwrap();
        assert_eq!(
            v,
            json!({"type":"transcript_final","turn_id":"voice_xyz","text":"hello world"})
        );
    }

    #[test]
    fn tts_end_serialize() {
        let msg = ServerMessage::TtsEnd {
            turn_id: "voice_xyz".into(),
        };
        let v = serde_json::to_value(&msg).unwrap();
        assert_eq!(v, json!({"type":"tts_end","turn_id":"voice_xyz"}));
    }

    #[test]
    fn truncate_serialize() {
        let msg = ServerMessage::Truncate {
            turn_id: "voice_xyz".into(),
            keep_ms: 1_500,
        };
        let v = serde_json::to_value(&msg).unwrap();
        assert_eq!(
            v,
            json!({"type":"truncate","turn_id":"voice_xyz","keep_ms":1500})
        );
    }

    #[test]
    fn tts_frame_header_round_trip() {
        let opus = vec![1_u8, 2, 3, 4, 5];
        let chunk_id: u64 = 0x0102_0304_0506_0708;
        let flags = FLAG_FILLER | FLAG_FIRST_OF_TURN;
        let framed = encode_tts_frame(chunk_id, flags, &opus);

        assert_eq!(framed.len(), TTS_FRAME_HEADER_LEN + opus.len());
        assert_eq!(
            &framed[0..8],
            &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]
        );
        assert_eq!(framed[8], flags);
        assert_eq!(&framed[9..], &opus[..]);
    }

    #[test]
    fn invalid_type_errors() {
        let raw = r#"{"type":"nonexistent_type"}"#;
        let result: Result<ClientMessage, _> = serde_json::from_str(raw);
        assert!(result.is_err());
    }
}
