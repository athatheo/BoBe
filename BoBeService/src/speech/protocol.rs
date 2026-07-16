//! WS wire protocol for `/voice/stream`. Client owns ASR + ships transcripts;
//! daemon owns LLM + TTS and ships Opus binary audio. No audio uplink.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ControlAction {
    Abort,
    Mute,
    Unmute,
    /// Drops running turn; keeps conversation context.
    Reset,
}

/// **Wire contract:** mirror `VoicePhaseWire` in Swift. Adding a variant
/// without the Swift counterpart breaks decode on the client.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum VoicePhase {
    Idle,
    Listening,
    Thinking,
    Speaking,
}

/// **Wire contract:** mirror `ClientVoiceMessage` in Swift; rename fields here = update both.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ClientMessage {
    Hello {
        session_id: String,
        playback_rate: u32,
        #[serde(default)]
        voice_id: Option<String>,
        #[serde(default)]
        speed: Option<f32>,
        /// BCP-47; daemon telemetry only — client picks the ASR engine.
        #[serde(default)]
        language: Option<String>,
        #[serde(default)]
        tts_backend: Option<String>,
    },
    /// Candidate barge-in; daemon decides after the min-words gate.
    BargeIn {
        ts_ms: u64,
        playback_ms_played: u64,
        /// Latest acoustic transcript evidence, carried atomically with the
        /// candidate so transport scheduling cannot reorder it behind barge-in.
        #[serde(default)]
        partial_text: Option<String>,
    },
    Wake {
        phrase: String,
        score: f32,
        ts_ms: u64,
    },
    /// Playback progress for truncation math.
    PlaybackAck {
        chunk_id: u64,
        played_ms: u64,
    },
    Control {
        action: ControlAction,
    },
    /// Daemon stores on `session.last_partial_text` for cancel-phrase + MinWords gate.
    TranscriptPartial {
        turn_id: String,
        text: String,
    },
    /// Triggers single-flight admit + convergence pipeline.
    TranscriptFinal {
        turn_id: String,
        text: String,
    },
    TtsPlaybackStarted {
        turn_id: String,
        synthesis_ms: u64,
    },
    TtsPlaybackComplete {
        turn_id: String,
    },
}

/// **Wire contract:** mirror `ServerVoiceMessage` in Swift. Binary TTS frames
/// bypass this enum — see `encode_tts_frame` below.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ServerMessage {
    HelloAck {
        voice_pack: String,
        playback_rate: u32,
    },
    State {
        phase: VoicePhase,
        turn_id: String,
    },
    TranscriptFinal {
        turn_id: String,
        text: String,
    },
    TtsEnd {
        turn_id: String,
    },
    TtsText {
        turn_id: String,
        sequence: u64,
        text: String,
    },
    /// Post-barge-in: client drops queued audio beyond `keep_ms`.
    Truncate {
        turn_id: String,
        keep_ms: u64,
    },
    Error {
        code: String,
        message: String,
    },
}

/// `[8 bytes BE u64 chunk_id][1 byte flags][N bytes Opus]`.
/// flags: bit 0 = filler (preemptible), bit 1 = first chunk of turn.
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
                tts_backend,
            } => {
                assert_eq!(session_id, "abc");
                assert_eq!(playback_rate, 24_000);
                assert_eq!(voice_id, None);
                assert_eq!(speed, None);
                assert_eq!(language, None);
                assert_eq!(tts_backend, None);
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
    fn client_tts_text_serializes_with_sequence() {
        let value = serde_json::to_value(ServerMessage::TtsText {
            turn_id: "turn-1".into(),
            sequence: 2,
            text: "Hello.".into(),
        })
        .unwrap();
        assert_eq!(value["type"], "tts_text");
        assert_eq!(value["turn_id"], "turn-1");
        assert_eq!(value["sequence"], 2);
        assert_eq!(value["text"], "Hello.");
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
                partial_text,
            } => {
                assert_eq!(ts_ms, 1_000);
                assert_eq!(playback_ms_played, 420);
                assert_eq!(partial_text, None);
            }
            other => panic!("expected BargeIn, got {other:?}"),
        }
    }

    #[test]
    fn barge_in_deserialize_with_atomic_partial_evidence() {
        let raw = r#"{
            "type":"barge_in",
            "ts_ms":1000,
            "playback_ms_played":420,
            "partial_text":"please stop now"
        }"#;
        let parsed: ClientMessage = serde_json::from_str(raw).unwrap();
        assert!(matches!(
            parsed,
            ClientMessage::BargeIn {
                partial_text: Some(ref text),
                ..
            } if text == "please stop now"
        ));
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
