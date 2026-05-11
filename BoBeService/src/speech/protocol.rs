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
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
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
pub(crate) enum ClientMessage {
    /// Session handshake; sent once after WS connect.
    Hello {
        session_id: String,
        capture_rate: u32,
        playback_rate: u32,
        codec: String,
    },
    /// Optional fast hint that mic energy crossed threshold.
    VadHint {
        kind: VadHintKind,
        rms_dbfs: f32,
        ts_ms: u64,
    },
    /// Client detected speech during BoBe TTS playback — candidate barge-in.
    /// Daemon decides whether to honour after the min-words gate.
    BargeIn {
        ts_ms: u64,
        playback_ms_played: u64,
    },
    /// Wake-word fired locally. Daemon may auto-open mic if not yet active.
    Wake {
        phrase: String,
        score: f32,
        ts_ms: u64,
    },
    /// Reports how much of TTS chunk_id has actually played, for truncation math.
    PlaybackAck {
        chunk_id: u64,
        played_ms: u64,
    },
    /// User-initiated control.
    Control {
        action: ControlAction,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
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

#[allow(dead_code, reason = "used by tts_pipeline in 0.d")]
pub(crate) fn encode_tts_frame(chunk_id: u64, flags: u8, opus: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(TTS_FRAME_HEADER_LEN + opus.len());
    buf.extend_from_slice(&chunk_id.to_be_bytes());
    buf.push(flags);
    buf.extend_from_slice(opus);
    buf
}
