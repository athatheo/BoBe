@testable import BoBe
import Foundation
import Testing

@Suite("voiceWsEndpoint(from:)")
struct VoiceWsEndpointTests {
    @Test
    func httpBecomesWs() throws {
        let url = try #require(URL(string: "http://127.0.0.1:8766"))
        let ws = voiceWsEndpoint(from: url)
        #expect(ws?.absoluteString == "ws://127.0.0.1:8766/voice/stream")
    }

    @Suite("VoiceSttLoadProgress")
    struct VoiceSttLoadProgressTests {
        @Test
        func percentRoundsAndClamps() {
            #expect(
                VoiceSttLoadProgress(
                    fractionCompleted: 0.426,
                    phase: .downloading(completedFiles: 2, totalFiles: 5)
                ).percent == 43
            )
            #expect(
                VoiceSttLoadProgress(
                    fractionCompleted: -1,
                    phase: .listing
                ).percent == 0
            )
            #expect(
                VoiceSttLoadProgress(
                    fractionCompleted: 2,
                    phase: .compiling(modelName: "encoder")
                ).percent == 100
            )
        }
    }

    @Test
    func httpsBecomesWss() throws {
        let url = try #require(URL(string: "https://example.test:8443"))
        let ws = voiceWsEndpoint(from: url)
        #expect(ws?.absoluteString == "wss://example.test:8443/voice/stream")
    }

    @Test
    func preservesPathPrefix() throws {
        let url = try #require(URL(string: "https://example.test/bobe/api"))
        let ws = voiceWsEndpoint(from: url)
        #expect(ws?.absoluteString == "wss://example.test/bobe/api/voice/stream")
    }

    @Test
    func avoidsDoubleSlashAfterTrailingPrefixSlash() throws {
        let url = try #require(URL(string: "https://example.test/bobe/"))
        let ws = voiceWsEndpoint(from: url)
        #expect(ws?.path == "/bobe/voice/stream")
    }

    @Test
    func preservesPort() throws {
        let url = try #require(URL(string: "http://127.0.0.1:8766"))
        let ws = voiceWsEndpoint(from: url)
        #expect(ws?.port == 8766)
    }

    @Test
    func unknownSchemeFallsBackToWs() throws {
        // Defensive: a non-http(s) scheme is unexpected but shouldn't crash.
        // Current behavior: lowercased scheme != "https" → ws.
        let url = try #require(URL(string: "weird://host:80"))
        let ws = voiceWsEndpoint(from: url)
        #expect(ws?.scheme == "ws")
    }
}
