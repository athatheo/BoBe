import Foundation
import Testing

@testable import BoBe

@Suite("voiceWsEndpoint(from:)")
struct VoiceWsEndpointTests {
    @Test func httpBecomesWs() {
        let url = URL(string: "http://127.0.0.1:8766")!
        let ws = voiceWsEndpoint(from: url)
        #expect(ws?.absoluteString == "ws://127.0.0.1:8766/voice/stream")
    }

    @Test func httpsBecomesWss() {
        let url = URL(string: "https://example.test:8443")!
        let ws = voiceWsEndpoint(from: url)
        #expect(ws?.absoluteString == "wss://example.test:8443/voice/stream")
    }

    @Test func pathIsOverwritten() {
        // Daemon base URL may carry a path (typically "/" or nothing) — the
        // function should replace it with /voice/stream regardless.
        let url = URL(string: "http://127.0.0.1:8766/api/v1")!
        let ws = voiceWsEndpoint(from: url)
        #expect(ws?.path == "/voice/stream")
    }

    @Test func preservesPort() {
        let url = URL(string: "http://127.0.0.1:8766")!
        let ws = voiceWsEndpoint(from: url)
        #expect(ws?.port == 8766)
    }

    @Test func unknownSchemeFallsBackToWs() {
        // Defensive: a non-http(s) scheme is unexpected but shouldn't crash.
        // Current behavior: lowercased scheme != "https" → ws.
        let url = URL(string: "weird://host:80")!
        let ws = voiceWsEndpoint(from: url)
        #expect(ws?.scheme == "ws")
    }
}
