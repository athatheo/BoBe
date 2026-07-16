@testable import BoBe
import Foundation
import Testing

/// Wire-shape tests for the daemon ↔ client voice protocol. Drift between
/// these values and `BoBeService/src/speech/protocol.rs` breaks every audio
/// frame, so the parser deserves explicit coverage.
@Suite("TtsFrameHeader")
struct TtsFrameHeaderTests {
    /// Build a frame the same way Rust's `encode_tts_frame` does: 8 bytes
    /// big-endian `chunk_id`, 1 byte `flags`, then payload.
    private func encode(chunkId: UInt64, flags: UInt8, payload: [UInt8]) -> Data {
        var data = Data(capacity: 9 + payload.count)
        for i in stride(from: 56, through: 0, by: -8) {
            data.append(UInt8((chunkId >> i) & 0xFF))
        }
        data.append(flags)
        data.append(contentsOf: payload)
        return data
    }

    @Test
    func parsesChunkIdBigEndian() {
        let frame = self.encode(chunkId: 0x0102_0304_0506_0708, flags: 0, payload: [0xAA, 0xBB])
        let parsed = TtsFrameHeader.parse(frame)
        #expect(parsed != nil)
        #expect(parsed?.header.chunkId == 0x0102_0304_0506_0708)
    }

    @Test
    func parsesEmptyPayload() {
        let frame = self.encode(chunkId: 42, flags: 0, payload: [])
        let parsed = TtsFrameHeader.parse(frame)
        #expect(parsed?.payload.count == 0)
        #expect(parsed?.header.chunkId == 42)
    }

    @Test
    func parsesPayloadPreservingBytes() {
        let payload: [UInt8] = [0x01, 0x02, 0x03, 0xFF, 0xFE]
        let frame = self.encode(chunkId: 1, flags: 0, payload: payload)
        let parsed = TtsFrameHeader.parse(frame)
        #expect(Array(parsed?.payload ?? Data()) == payload)
    }

    @Test
    func rejectsTooShort() {
        // 8 bytes is one short of the 9-byte header.
        let bad = Data([0, 0, 0, 0, 0, 0, 0, 1])
        #expect(TtsFrameHeader.parse(bad) == nil)
    }

    @Test
    func fillerFlagDetected() {
        let frame = self.encode(chunkId: 1, flags: TtsFrameHeader.flagFiller, payload: [])
        let parsed = TtsFrameHeader.parse(frame)
        #expect(parsed?.header.isFiller == true)
        #expect(parsed?.header.isFirstOfTurn == false)
    }

    @Test
    func firstOfTurnFlagDetected() {
        let frame = self.encode(chunkId: 1, flags: TtsFrameHeader.flagFirstOfTurn, payload: [])
        let parsed = TtsFrameHeader.parse(frame)
        #expect(parsed?.header.isFirstOfTurn == true)
        #expect(parsed?.header.isFiller == false)
    }

    @Test
    func bothFlagsCoexist() {
        let flags = TtsFrameHeader.flagFiller | TtsFrameHeader.flagFirstOfTurn
        let frame = self.encode(chunkId: 1, flags: flags, payload: [])
        let parsed = TtsFrameHeader.parse(frame)
        #expect(parsed?.header.isFiller == true)
        #expect(parsed?.header.isFirstOfTurn == true)
    }
}

@Suite("ServerVoiceMessage")
struct ServerVoiceMessageTests {
    private func decode(_ json: String) throws -> ServerVoiceMessage {
        let data = json.data(using: .utf8) ?? Data()
        return try JSONDecoder().decode(ServerVoiceMessage.self, from: data)
    }

    @Test
    func decodesStateMessage() throws {
        let msg = try decode(#"{"type":"state","phase":"speaking","turn_id":"abc"}"#)
        guard case let .state(phase, turnId) = msg else {
            Issue.record("expected .state, got \(msg)")
            return
        }
        #expect(phase == .speaking)
        #expect(turnId == "abc")
    }

    @Test
    func decodesTruncate() throws {
        let msg = try decode(#"{"type":"truncate","turn_id":"t1","keep_ms":1500}"#)
        guard case let .truncate(turnId, keepMs) = msg else {
            Issue.record("expected .truncate, got \(msg)")
            return
        }
        #expect(turnId == "t1")
        #expect(keepMs == 1500)
    }

    @Test
    func decodesClientTtsText() throws {
        let msg = try decode(
            #"{"type":"tts_text","turn_id":"t1","sequence":2,"text":"Hello."}"#
        )
        guard case let .ttsText(turnId, sequence, text) = msg else {
            Issue.record("expected .ttsText, got \(msg)")
            return
        }
        #expect(turnId == "t1")
        #expect(sequence == 2)
        #expect(text == "Hello.")
    }

    @Test
    func decodesError() throws {
        let msg = try decode(#"{"type":"error","code":"PERMIT_REJECTED","message":"single-flight"}"#)
        guard case let .error(code, message) = msg else {
            Issue.record("expected .error, got \(msg)")
            return
        }
        #expect(code == "PERMIT_REJECTED")
        #expect(message == "single-flight")
    }

    @Test
    func unknownTypeBecomesUnknownVariant() throws {
        // Forward-compat: a daemon adding a new server message shouldn't make
        // the client throw — it should fall through to `.unknown` and be
        // ignored at the dispatch layer.
        let msg = try decode(#"{"type":"future_event","data":42}"#)
        guard case let .unknown(type) = msg else {
            Issue.record("expected .unknown, got \(msg)")
            return
        }
        #expect(type == "future_event")
    }
}

@Suite("ClientVoiceMessage encoding")
struct ClientVoiceMessageEncodingTests {
    private func encode(_ msg: ClientVoiceMessage) throws -> [String: Any] {
        let data = try JSONEncoder().encode(msg)
        let any = try JSONSerialization.jsonObject(with: data)
        return any as? [String: Any] ?? [:]
    }

    @Test
    func controlEncodesAbort() throws {
        let obj = try encode(.control(action: .abort))
        #expect(obj["type"] as? String == "control")
        #expect(obj["action"] as? String == "abort")
    }

    @Test
    func transcriptPartialEncodesSnakeCaseKeys() throws {
        let obj = try encode(.transcriptPartial(turnId: "abc", text: "hi"))
        #expect(obj["type"] as? String == "transcript_partial")
        // Wire contract: turn_id, not turnId. Daemon decodes via serde
        // snake_case, so the Swift CodingKey rename must match.
        #expect(obj["turn_id"] as? String == "abc")
        #expect(obj["text"] as? String == "hi")
    }

    @Test
    func bargeInCarriesAtomicPartialEvidence() throws {
        let obj = try encode(.bargeIn(
            tsMs: 1_000,
            playbackMsPlayed: 420,
            partialText: "please stop now"
        ))
        #expect(obj["type"] as? String == "barge_in")
        #expect(obj["playback_ms_played"] as? Int == 420)
        #expect(obj["partial_text"] as? String == "please stop now")
    }

    @Test
    func helloEncodesAllOptionalsWhenSet() throws {
        let obj = try encode(.hello(
            sessionId: "s1",
            playbackRate: 24_000,
            voiceId: "af_alloy",
            speed: 1.0,
            language: "en",
            ttsBackend: "client_supertonic"
        ))
        #expect(obj["session_id"] as? String == "s1")
        #expect(obj["playback_rate"] as? Int == 24_000)
        #expect(obj["voice_id"] as? String == "af_alloy")
        #expect(obj["language"] as? String == "en")
        #expect(obj["tts_backend"] as? String == "client_supertonic")
    }

    @Test
    func helloOmitsNilOptionals() throws {
        let obj = try encode(.hello(
            sessionId: "s1",
            playbackRate: 24_000,
            voiceId: nil,
            speed: nil,
            language: nil,
            ttsBackend: nil
        ))
        #expect(obj["voice_id"] == nil)
        #expect(obj["speed"] == nil)
        #expect(obj["language"] == nil)
        #expect(obj["tts_backend"] == nil)
    }

    @Test
    func clientTtsCompletionEncodes() throws {
        let obj = try encode(.ttsPlaybackComplete(turnId: "turn-1"))
        #expect(obj["type"] as? String == "tts_playback_complete")
        #expect(obj["turn_id"] as? String == "turn-1")
    }
}
