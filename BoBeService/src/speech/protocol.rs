//! WS wire protocol for `/voice/stream`. Deepgram-style: binary frames carry
//! Opus packets, text JSON on the same socket carries control.
//!
//! See `docs/voice-plan.md` §3 for the full 13-message contract:
//!   6 client→daemon (json) + 6 daemon→client (json) + 2 binary frame types.

use serde::{Deserialize, Serialize};

/// Speech-activity hint from client (cheap RMS gate). Daemon-side Silero VAD is
/// authoritative; these are bandwidth-saver signals only.
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum VadHintKind {
    SpeechStart,
    SpeechEnd,
}

/// Control actions the client may request mid-session.
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ControlAction {
    /// Cancel the in-flight turn (user-initiated barge-in or explicit abort).
    Abort,
    /// Pause mic uplink — daemon stops accepting `audio.in` until Unmute.
    Mute,
    Unmute,
    /// Reset session state (clear running turn; conversation context preserved).
    Reset,
}

/// Daemon-side authoritative turn phase. Client mirrors for UI only.
///
/// Some variants are not yet emitted by the daemon — they're part of the
/// protocol contract for future milestones (Capturing on partial-speech UX,
/// Cancelling on M4.5.5 barge-in, Failed on engine-load errors).
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code, reason = "Capturing/Cancelling/Failed reserved for M4.5.5+")]
pub(crate) enum VoicePhase {
    Idle,
    Listening,
    Capturing,
    Thinking,
    Speaking,
    Cancelling,
    Failed,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(
    dead_code,
    reason = "VadHint/BargeIn/Wake/PlaybackAck fields are wire-protocol contract; consumed in M4.5.5+"
)]
pub(crate) enum ClientMessage {
    /// Session handshake; sent once after WS connect. Optional voice fields
    /// let the client express per-session voice preferences (Kokoro voice
    /// slot, speed) that override the daemon's DaemonSettings defaults for
    /// this WS only. Absence falls through to the daemon defaults.
    Hello {
        session_id: String,
        capture_rate: u32,
        playback_rate: u32,
        codec: String,
        #[serde(default)]
        voice_id: Option<String>,
        #[serde(default)]
        speed: Option<f32>,
        #[serde(default)]
        voice_pack: Option<String>,
    },
    /// Optional fast hint that mic energy crossed threshold.
    VadHint {
        kind: VadHintKind,
        rms_dbfs: f32,
        ts_ms: u64,
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
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(
    dead_code,
    reason = "TranscriptPartial reserved for streaming-STT; Truncate reserved for M4.5.5 barge-in"
)]
pub(crate) enum ServerMessage {
    /// Authoritative phase transition. Sent on every state change.
    State {
        phase: VoicePhase,
        turn_id: String,
    },
    /// Optional partial transcript (only emitted when streaming STT lands).
    TranscriptPartial {
        turn_id: String,
        text: String,
    },
    /// Confirmed final transcript after smart-turn + STT.
    TranscriptFinal {
        turn_id: String,
        text: String,
    },
    /// All sentences flushed for this turn — client may leave Speaking.
    TtsEnd {
        turn_id: String,
    },
    /// Post-barge-in: client drops queued audio beyond `keep_ms`.
    Truncate {
        turn_id: String,
        keep_ms: u64,
    },
    /// Non-fatal error message.
    Error {
        code: String,
        message: String,
    },
}

/// Binary frame layout for `tts.chunk` and `filler.chunk`:
/// `[8 bytes BE u64 chunk_id][1 byte flags][N bytes Opus packet]`.
///
/// flags bit 0: 1 = filler (preemptible by real reply)
/// flags bit 1: 1 = first chunk of turn
/// flags bit 2: 1 = last chunk of turn
pub(crate) const TTS_FRAME_HEADER_LEN: usize = 9;
#[allow(dead_code, reason = "used by tts_pipeline in 0.d / fillers in M4.5.4")]
pub(crate) const FLAG_FILLER: u8 = 0b0000_0001;
#[allow(dead_code, reason = "used by tts_pipeline in 0.d")]
pub(crate) const FLAG_FIRST_OF_TURN: u8 = 0b0000_0010;
#[allow(dead_code, reason = "used by tts_pipeline in 0.d")]
pub(crate) const FLAG_LAST_OF_TURN: u8 = 0b0000_0100;

pub(crate) fn encode_tts_frame(chunk_id: u64, flags: u8, opus: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(TTS_FRAME_HEADER_LEN + opus.len());
    buf.extend_from_slice(&chunk_id.to_be_bytes());
    buf.push(flags);
    buf.extend_from_slice(opus);
    buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn hello_deserialize_minimal() {
        let raw = r#"{"type":"hello","session_id":"abc","capture_rate":16000,"playback_rate":24000,"codec":"opus"}"#;
        let parsed: ClientMessage = serde_json::from_str(raw).unwrap();
        match parsed {
            ClientMessage::Hello {
                session_id,
                capture_rate,
                playback_rate,
                codec,
                voice_id,
                speed,
                voice_pack,
            } => {
                assert_eq!(session_id, "abc");
                assert_eq!(capture_rate, 16_000);
                assert_eq!(playback_rate, 24_000);
                assert_eq!(codec, "opus");
                assert_eq!(voice_id, None);
                assert_eq!(speed, None);
                assert_eq!(voice_pack, None);
            }
            other => panic!("expected Hello, got {other:?}"),
        }
    }

    #[test]
    fn hello_deserialize_with_voice_cfg() {
        let raw = r#"{
            "type":"hello",
            "session_id":"abc",
            "capture_rate":16000,
            "playback_rate":24000,
            "codec":"opus",
            "voice_id":"am_michael",
            "speed":1.2,
            "voice_pack":"warm"
        }"#;
        let parsed: ClientMessage = serde_json::from_str(raw).unwrap();
        match parsed {
            ClientMessage::Hello {
                voice_id,
                speed,
                voice_pack,
                ..
            } => {
                assert_eq!(voice_id, Some("am_michael".into()));
                assert_eq!(speed, Some(1.2));
                assert_eq!(voice_pack, Some("warm".into()));
            }
            other => panic!("expected Hello, got {other:?}"),
        }
    }

    #[test]
    fn vad_hint_deserialize() {
        let raw = r#"{"type":"vad_hint","kind":"speech_start","rms_dbfs":-30.5,"ts_ms":1234}"#;
        let parsed: ClientMessage = serde_json::from_str(raw).unwrap();
        assert!(matches!(
            parsed,
            ClientMessage::VadHint {
                kind: VadHintKind::SpeechStart,
                ..
            }
        ));
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
    fn state_serialize_uses_snake_case() {
        let msg = ServerMessage::State {
            phase: VoicePhase::Speaking,
            turn_id: "voice_xyz".into(),
        };
        let v = serde_json::to_value(&msg).unwrap();
        assert_eq!(v, json!({"type":"state","phase":"speaking","turn_id":"voice_xyz"}));
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
        // Big-endian u64 header
        assert_eq!(
            &framed[0..8],
            &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]
        );
        assert_eq!(framed[8], flags);
        assert_eq!(&framed[9..], &opus[..]);
    }

    #[test]
    fn voice_phase_serializes_snake_case() {
        for (phase, want) in [
            (VoicePhase::Idle, "idle"),
            (VoicePhase::Listening, "listening"),
            (VoicePhase::Capturing, "capturing"),
            (VoicePhase::Thinking, "thinking"),
            (VoicePhase::Speaking, "speaking"),
            (VoicePhase::Cancelling, "cancelling"),
            (VoicePhase::Failed, "failed"),
        ] {
            let v = serde_json::to_value(phase).unwrap();
            assert_eq!(v.as_str(), Some(want));
        }
    }

    #[test]
    fn invalid_type_errors() {
        let raw = r#"{"type":"nonexistent_type"}"#;
        let result: Result<ClientMessage, _> = serde_json::from_str(raw);
        assert!(result.is_err());
    }
}
