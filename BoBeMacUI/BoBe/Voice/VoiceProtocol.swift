// Wire-protocol Codable types for `/voice/stream`. Mirrors
// BoBeService/src/speech/protocol.rs (snake_case JSON). Binary frames are
// handled separately in VoicePipeline (9-byte header + Opus packet).

import Foundation

enum VoiceVadHintKind: String, Codable {
    case speechStart = "speech_start"
    case speechEnd = "speech_end"
}

enum VoiceControlAction: String, Codable {
    case abort
    case mute
    case unmute
    case reset
}

enum VoicePhaseWire: String, Codable {
    case idle
    case listening
    case capturing
    case thinking
    case speaking
    case cancelling
    case failed
}

enum ClientVoiceMessage: Codable {
    case hello(sessionId: String, captureRate: UInt32, playbackRate: UInt32, codec: String)
    case vadHint(kind: VoiceVadHintKind, rmsDbfs: Float, tsMs: UInt64)
    case bargeIn(tsMs: UInt64, playbackMsPlayed: UInt64)
    case wake(phrase: String, score: Float, tsMs: UInt64)
    case playbackAck(chunkId: UInt64, playedMs: UInt64)
    case control(action: VoiceControlAction)

    func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case let .hello(sessionId, captureRate, playbackRate, codec):
            try c.encode("hello", forKey: .type)
            try c.encode(sessionId, forKey: .sessionId)
            try c.encode(captureRate, forKey: .captureRate)
            try c.encode(playbackRate, forKey: .playbackRate)
            try c.encode(codec, forKey: .codec)
        case let .vadHint(kind, rmsDbfs, tsMs):
            try c.encode("vad_hint", forKey: .type)
            try c.encode(kind, forKey: .kind)
            try c.encode(rmsDbfs, forKey: .rmsDbfs)
            try c.encode(tsMs, forKey: .tsMs)
        case let .bargeIn(tsMs, playbackMsPlayed):
            try c.encode("barge_in", forKey: .type)
            try c.encode(tsMs, forKey: .tsMs)
            try c.encode(playbackMsPlayed, forKey: .playbackMsPlayed)
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
        }
    }

    init(from _: Decoder) throws {
        // Client-side encodes only — decode is not needed for outbound messages.
        throw DecodingError.dataCorrupted(
            DecodingError.Context(codingPath: [], debugDescription: "ClientVoiceMessage is encode-only")
        )
    }

    enum CodingKeys: String, CodingKey {
        case type
        case sessionId = "session_id"
        case captureRate = "capture_rate"
        case playbackRate = "playback_rate"
        case codec
        case kind
        case rmsDbfs = "rms_dbfs"
        case tsMs = "ts_ms"
        case playbackMsPlayed = "playback_ms_played"
        case phrase
        case score
        case chunkId = "chunk_id"
        case playedMs = "played_ms"
        case action
    }
}

enum ServerVoiceMessage: Decodable {
    case state(phase: VoicePhaseWire, turnId: String)
    case transcriptPartial(turnId: String, text: String)
    case transcriptFinal(turnId: String, text: String)
    case ttsEnd(turnId: String)
    case truncate(turnId: String, keepMs: UInt64)
    case error(code: String, message: String)
    case unknown(type: String)

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        let type = try c.decode(String.self, forKey: .type)
        switch type {
        case "state":
            self = .state(
                phase: try c.decode(VoicePhaseWire.self, forKey: .phase),
                turnId: try c.decode(String.self, forKey: .turnId)
            )
        case "transcript_partial":
            self = .transcriptPartial(
                turnId: try c.decode(String.self, forKey: .turnId),
                text: try c.decode(String.self, forKey: .text)
            )
        case "transcript_final":
            self = .transcriptFinal(
                turnId: try c.decode(String.self, forKey: .turnId),
                text: try c.decode(String.self, forKey: .text)
            )
        case "tts_end":
            self = .ttsEnd(turnId: try c.decode(String.self, forKey: .turnId))
        case "truncate":
            self = .truncate(
                turnId: try c.decode(String.self, forKey: .turnId),
                keepMs: try c.decode(UInt64.self, forKey: .keepMs)
            )
        case "error":
            self = .error(
                code: try c.decode(String.self, forKey: .code),
                message: try c.decode(String.self, forKey: .message)
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
    }
}

/// Binary frame header for `tts.chunk` / `filler.chunk` (daemon→client):
/// 8 bytes BE u64 chunk_id, then 1 byte flags, then Opus packet.
struct TtsFrameHeader {
    let chunkId: UInt64
    let flags: UInt8

    static let length = 9

    static func parse(_ data: Data) -> (header: TtsFrameHeader, payload: Data)? {
        guard data.count >= length else { return nil }
        var chunkId: UInt64 = 0
        for i in 0..<8 {
            chunkId = (chunkId << 8) | UInt64(data[data.startIndex + i])
        }
        let flags = data[data.startIndex + 8]
        let payload = data.subdata(in: (data.startIndex + length)..<data.endIndex)
        return (TtsFrameHeader(chunkId: chunkId, flags: flags), payload)
    }

    var isFiller: Bool { flags & 0b0000_0001 != 0 }
    var isFirstOfTurn: Bool { flags & 0b0000_0010 != 0 }
    var isLastOfTurn: Bool { flags & 0b0000_0100 != 0 }
}
