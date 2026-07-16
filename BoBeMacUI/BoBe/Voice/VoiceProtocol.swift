import Foundation

enum VoiceControlAction: String, Codable {
    case abort
    case mute
    case unmute
    case reset
}

/// Subset of `VoicePipeline.State` that's authoritative from the daemon.
/// The client-only states (`.connecting`, `.cancelling`, `.failed`,
/// `.capturing`) are driven from local audio/WS events, not the wire.
///
/// **Wire contract:** match `BoBeService/src/speech/protocol.rs::VoicePhase`
/// variant set. Adding a daemon-side variant without the Swift counterpart
/// makes this decode throw — clients drop the whole state message.
enum VoicePhaseWire: String, Codable {
    case idle
    case listening
    case thinking
    case speaking
}

/// **Wire contract:** the `type` discriminator values encoded by this
/// enum must match `BoBeService/src/speech/protocol.rs::ClientMessage`
/// serde variant snake_case form, AND each variant's `CodingKeys` must
/// align with the Rust struct field set. Drift here makes the daemon
/// reject the JSON at deserialization.
enum ClientVoiceMessage: Codable {
    case hello(
        sessionId: String,
        playbackRate: UInt32,
        voiceId: String?,
        speed: Float?,
        language: String?,
        ttsBackend: String?
    )
    case bargeIn(tsMs: UInt64, playbackMsPlayed: UInt64, partialText: String?)
    case wake(phrase: String, score: Float, tsMs: UInt64)
    case playbackAck(chunkId: UInt64, playedMs: UInt64)
    case control(action: VoiceControlAction)
    /// Streaming partial transcript from the client's local ASR. Daemon
    /// stores it for cancel-phrase + MinWords barge-in gating.
    case transcriptPartial(turnId: String, text: String)
    /// Finalized transcript at end-of-utterance. Daemon admits the turn
    /// and runs the LLM + TTS pipeline.
    case transcriptFinal(turnId: String, text: String)
    case ttsPlaybackStarted(turnId: String, synthesisMs: UInt64)
    case ttsPlaybackComplete(turnId: String)

    func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case let .hello(sessionId, playbackRate, voiceId, speed, language, ttsBackend):
            try c.encode("hello", forKey: .type)
            try c.encode(sessionId, forKey: .sessionId)
            try c.encode(playbackRate, forKey: .playbackRate)
            try c.encodeIfPresent(voiceId, forKey: .voiceId)
            try c.encodeIfPresent(speed, forKey: .speed)
            try c.encodeIfPresent(language, forKey: .language)
            try c.encodeIfPresent(ttsBackend, forKey: .ttsBackend)
        case let .bargeIn(tsMs, playbackMsPlayed, partialText):
            try c.encode("barge_in", forKey: .type)
            try c.encode(tsMs, forKey: .tsMs)
            try c.encode(playbackMsPlayed, forKey: .playbackMsPlayed)
            try c.encodeIfPresent(partialText, forKey: .partialText)
        case let .wake(phrase, score, tsMs):
            try c.encode("wake", forKey: .type)
            try c.encode(phrase, forKey: .phrase)
            try c.encode(score, forKey: .score)
            try c.encode(tsMs, forKey: .tsMs)
        case let .playbackAck(chunkId, playedMs):
            try c.encode("playback_ack", forKey: .type)
            try c.encode(chunkId, forKey: .chunkId)
            try c.encode(playedMs, forKey: .playedMs)
        case let .control(action):
            try c.encode("control", forKey: .type)
            try c.encode(action, forKey: .action)
        case let .transcriptPartial(turnId, text):
            try c.encode("transcript_partial", forKey: .type)
            try c.encode(turnId, forKey: .turnId)
            try c.encode(text, forKey: .text)
        case let .transcriptFinal(turnId, text):
            try c.encode("transcript_final", forKey: .type)
            try c.encode(turnId, forKey: .turnId)
            try c.encode(text, forKey: .text)
        case let .ttsPlaybackStarted(turnId, synthesisMs):
            try c.encode("tts_playback_started", forKey: .type)
            try c.encode(turnId, forKey: .turnId)
            try c.encode(synthesisMs, forKey: .synthesisMs)
        case let .ttsPlaybackComplete(turnId):
            try c.encode("tts_playback_complete", forKey: .type)
            try c.encode(turnId, forKey: .turnId)
        }
    }

    init(from _: Decoder) throws {
        // Client encodes only — outbound messages aren't decoded locally.
        throw DecodingError.dataCorrupted(
            DecodingError.Context(codingPath: [], debugDescription: "ClientVoiceMessage is encode-only")
        )
    }

    enum CodingKeys: String, CodingKey {
        case type
        case sessionId = "session_id"
        case playbackRate = "playback_rate"
        case voiceId = "voice_id"
        case speed
        case language
        case ttsBackend = "tts_backend"
        case tsMs = "ts_ms"
        case playbackMsPlayed = "playback_ms_played"
        case partialText = "partial_text"
        case phrase
        case score
        case chunkId = "chunk_id"
        case playedMs = "played_ms"
        case action
        case turnId = "turn_id"
        case text
        case synthesisMs = "synthesis_ms"
    }
}

/// **Wire contract:** mirrors `BoBeService/src/speech/protocol.rs::
/// ServerMessage`. Each `case` corresponds to a Rust variant with the
/// `type` tag in snake_case. Binary TTS frames (9-byte header + Opus
/// payload) do NOT come through this enum — see `TtsFrameHeader` below.
enum ServerVoiceMessage: Decodable {
    case helloAck(voicePack: String, playbackRate: UInt32)
    case state(phase: VoicePhaseWire, turnId: String)
    case transcriptFinal(turnId: String, text: String)
    case ttsEnd(turnId: String)
    case ttsText(turnId: String, sequence: UInt64, text: String)
    case truncate(turnId: String, keepMs: UInt64)
    case error(code: String, message: String)
    case unknown(type: String)

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        let type = try c.decode(String.self, forKey: .type)
        switch type {
        case "hello_ack":
            self = try .helloAck(
                voicePack: c.decode(String.self, forKey: .voicePack),
                playbackRate: c.decode(UInt32.self, forKey: .playbackRate)
            )
        case "state":
            self = try .state(
                phase: c.decode(VoicePhaseWire.self, forKey: .phase),
                turnId: c.decode(String.self, forKey: .turnId)
            )
        case "transcript_final":
            self = try .transcriptFinal(
                turnId: c.decode(String.self, forKey: .turnId),
                text: c.decode(String.self, forKey: .text)
            )
        case "tts_end":
            self = try .ttsEnd(turnId: c.decode(String.self, forKey: .turnId))
        case "tts_text":
            self = try .ttsText(
                turnId: c.decode(String.self, forKey: .turnId),
                sequence: c.decode(UInt64.self, forKey: .sequence),
                text: c.decode(String.self, forKey: .text)
            )
        case "truncate":
            self = try .truncate(
                turnId: c.decode(String.self, forKey: .turnId),
                keepMs: c.decode(UInt64.self, forKey: .keepMs)
            )
        case "error":
            self = try .error(
                code: c.decode(String.self, forKey: .code),
                message: c.decode(String.self, forKey: .message)
            )
        default:
            self = .unknown(type: type)
        }
    }

    enum CodingKeys: String, CodingKey {
        case type
        case phase
        case turnId = "turn_id"
        case text
        case keepMs = "keep_ms"
        case code
        case message
        case voicePack = "voice_pack"
        case playbackRate = "playback_rate"
        case sequence
    }
}

/// Binary frame header for `tts.chunk` / `filler.chunk` (daemon→client):
/// 8 bytes BE u64 chunk_id, then 1 byte flags, then Opus packet. Match
/// Rust `speech::protocol::{TTS_FRAME_HEADER_LEN, FLAG_FILLER,
/// FLAG_FIRST_OF_TURN, encode_tts_frame}`. The drift script asserts the
/// three constants stay in lockstep.
struct TtsFrameHeader {
    let chunkId: UInt64
    let flags: UInt8

    static let length = 9
    static let flagFiller: UInt8 = 0b0000_0001
    static let flagFirstOfTurn: UInt8 = 0b0000_0010

    static func parse(_ data: Data) -> (header: TtsFrameHeader, payload: Data)? {
        guard data.count >= self.length else { return nil }
        var chunkId: UInt64 = 0
        for i in 0 ..< 8 {
            chunkId = (chunkId << 8) | UInt64(data[data.startIndex + i])
        }
        let flags = data[data.startIndex + 8]
        let payload = data.subdata(in: (data.startIndex + self.length) ..< data.endIndex)
        return (TtsFrameHeader(chunkId: chunkId, flags: flags), payload)
    }

    var isFiller: Bool {
        self.flags & Self.flagFiller != 0
    }

    var isFirstOfTurn: Bool {
        self.flags & Self.flagFirstOfTurn != 0
    }
}
