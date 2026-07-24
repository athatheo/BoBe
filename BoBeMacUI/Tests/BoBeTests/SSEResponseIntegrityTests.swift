@testable import BoBe
import Foundation
import Testing

@Suite("SSE response integrity", .serialized)
struct SSEResponseIntegrityTests {
    @Test
    @MainActor
    func terminalDeltaReplacesIncompleteStreamWithAuthoritativeResponse() async throws {
        let store = BobeStore.shared
        store.clearMessages()
        defer { store.clearMessages() }

        await store.processBundle(try Self.bundle(delta: "Hel", done: false))
        await store.processBundle(try Self.bundle(delta: "Hello, complete response", done: true))

        let message = try #require(store.messages.first(where: { $0.id == "assistant-1" }))
        #expect(message.content == "Hello, complete response")
        #expect(!message.isStreaming)
        #expect(message.isComplete)
        #expect(store.latestLiveBobeMessageId == "assistant-1")
    }

    @Test
    @MainActor
    func ambientReplyEventWaitsForVisibleLiveContent() async throws {
        let store = BobeStore.shared
        store.clearMessages()
        defer { store.clearMessages() }

        await store.processBundle(try Self.bundle(id: "assistant-2", delta: " \n", done: false))
        #expect(store.latestLiveBobeMessageId == nil)

        await store.processBundle(try Self.bundle(id: "assistant-2", delta: "Hello", done: true))
        #expect(store.latestLiveBobeMessageId == "assistant-2")
    }

    @Test
    @MainActor
    func recoverableChatErrorPreservesInterruptedResponse() async throws {
        let store = BobeStore.shared
        store.clearMessages()
        defer { store.clearMessages() }

        await store.processBundle(try Self.bundle(id: "assistant-3", delta: "Partial response", done: false))
        await store.processBundle(try Self.errorBundle(id: "assistant-3"))

        let message = try #require(store.messages.first(where: { $0.id == "assistant-3" }))
        #expect(message.content == "Partial response")
        #expect(!message.isStreaming)
        #expect(!message.isComplete)
    }

    @Test
    @MainActor
    func canonicalSnapshotKeepsWireIdentityAndInterruptedState() throws {
        let json = """
        {
          "conversation_id": "conversation-1",
          "turns": [
            {
              "id": "persisted-user",
              "message_id": "msg_user",
              "role": "user",
              "content": "Hello",
              "is_complete": true,
              "created_at": "2026-01-01T00:00:00Z"
            },
            {
              "id": "persisted-assistant",
              "message_id": "msg_assistant",
              "role": "assistant",
              "content": "Interrupted response",
              "is_complete": false,
              "created_at": "2026-01-01T00:00:01Z"
            }
          ]
        }
        """
        let snapshot = try JSONDecoder().decode(
            ConversationSnapshotResponse.self,
            from: Data(json.utf8)
        )

        let messages = BobeStore.canonicalMessages(from: snapshot.turns)

        #expect(messages.map(\.id) == ["msg_user", "msg_assistant"])
        #expect(messages[1].responseOrigin == .userInitiated)
        #expect(!messages[1].isComplete)
    }

    @Test
    @MainActor
    func canonicalReplacementFreezesPendingDeltaBeforeAwaitingSnapshot() async throws {
        let store = BobeStore.shared
        store.clearMessages()
        defer { store.clearMessages() }

        await store.processBundle(try Self.bundle(id: "assistant-pending", delta: "Visible partial", done: false))
        #expect(store.textDeltaFlushTask != nil)

        let interrupted = try #require(store.freezeActiveStreamBeforeCanonicalReplacement())

        let message = try #require(store.messages.first(where: { $0.id == "assistant-pending" }))
        #expect(message.content == "Visible partial")
        #expect(!message.isStreaming)
        #expect(!message.isComplete)
        #expect(interrupted.id == message.id)
        #expect(interrupted.content == message.content)
        #expect(store.streamingMessageId == nil)
        #expect(store.context.currentMessage.isEmpty)
        #expect(store.textDeltaFlushTask == nil)
    }

    @Test
    @MainActor
    func canonicalReplacementDropsEmptyStreamingPlaceholder() async throws {
        let store = BobeStore.shared
        store.clearMessages()
        defer { store.clearMessages() }

        await store.processBundle(try Self.bundle(id: "assistant-empty", delta: " \n", done: false))

        #expect(store.freezeActiveStreamBeforeCanonicalReplacement() == nil)
        #expect(!store.messages.contains(where: { $0.id == "assistant-empty" }))
        #expect(store.context.lastMessage == nil)
        #expect(store.streamingMessageId == nil)
    }

    private static func bundle(
        id: String = "assistant-1",
        delta: String,
        done: Bool
    ) throws -> StreamBundle {
        let json = """
        {
          "type": "text_delta",
          "message_id": \(String(reflecting: id)),
          "payload": {
            "delta": \(String(reflecting: delta)),
            "sequence": 1,
            "done": \(done)
          }
        }
        """
        return try JSONDecoder().decode(StreamBundle.self, from: Data(json.utf8))
    }

    private static func errorBundle(id: String) throws -> StreamBundle {
        let json = """
        {
          "type": "error",
          "message_id": \(String(reflecting: id)),
          "payload": {
            "code": "CHAT_STREAM_TIMEOUT",
            "message": "Assistant stream stopped responding",
            "recoverable": true
          }
        }
        """
        return try JSONDecoder().decode(StreamBundle.self, from: Data(json.utf8))
    }
}
