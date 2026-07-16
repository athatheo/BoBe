@testable import BoBe
import Foundation
import Testing

@Suite("SSE response integrity", .serialized)
struct SSEResponseIntegrityTests {
    @Test
    @MainActor
    func terminalDeltaReplacesIncompleteStreamWithAuthoritativeResponse() throws {
        let store = BobeStore.shared
        store.clearMessages()

        store.processBundle(try Self.bundle(delta: "Hel", done: false))
        store.processBundle(try Self.bundle(delta: "Hello, complete response", done: true))

        let message = try #require(store.messages.first(where: { $0.id == "assistant-1" }))
        #expect(message.content == "Hello, complete response")
        #expect(!message.isStreaming)
    }

    private static func bundle(delta: String, done: Bool) throws -> StreamBundle {
        let json = """
        {
          "type": "text_delta",
          "message_id": "assistant-1",
          "payload": {
            "delta": \(String(reflecting: delta)),
            "sequence": 1,
            "done": \(done)
          }
        }
        """
        return try JSONDecoder().decode(StreamBundle.self, from: Data(json.utf8))
    }
}
