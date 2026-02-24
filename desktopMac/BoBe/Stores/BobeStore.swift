import Foundation
import Observation
import OSLog

private let logger = Logger(subsystem: "com.bobe.app", category: "BobeStore")

/// Central observable state store for the BoBe app.
/// Replaces the Electron useSyncExternalStore + bobeActions pattern.
@Observable @MainActor
final class BobeStore {
    static let shared = BobeStore()

    // MARK: - State

    private(set) var context = BobeContext()

    var stateType: BobeStateType { context.stateType }
    var isConnected: Bool { context.daemonConnected }
    var isCapturing: Bool { context.capturing }
    var isThinking: Bool { context.thinking }
    var isSpeaking: Bool { context.speaking }
    var hasMessage: Bool { context.lastMessage != nil }
    var lastMessage: String? { context.lastMessage }
    var currentMessage: String { context.currentMessage }
    var currentMessageId: String? { context.currentMessageId }
    var messages: [ChatMessage] { context.messages }
    var activeIndicator: IndicatorType? { context.activeIndicator }
    var toolExecutions: [ToolExecution] { context.toolExecutions }
    var runningTools: [ToolExecution] { context.toolExecutions.filter { $0.status == .running } }

    // MARK: - Private

    private let client = DaemonClient.shared
    private var streamingMessage = ""
    private var streamingMessageId: String?
    private var lastMessageTimer: Task<Void, Never>?

    private init() {}

    // MARK: - Connection

    func connect() {
        Task {
            await client.connectSSE(
                onEvent: { [weak self] bundle in
                    Task { @MainActor in
                        self?.processBundle(bundle)
                    }
                },
                onConnectionChange: { [weak self] connected in
                    Task { @MainActor in
                        self?.updateState { $0.daemonConnected = connected }
                        if connected {
                            // Clear stale state on reconnect
                            self?.updateState {
                                $0.lastMessage = nil
                                $0.currentMessage = ""
                                $0.currentMessageId = nil
                                $0.speaking = false
                            }
                        }
                    }
                }
            )
        }
    }

    func disconnect() {
        Task {
            await client.disconnectSSE()
        }
    }

    // MARK: - Actions

    func toggleCapture() async -> Bool {
        let newState = !context.capturing
        do {
            if newState {
                try await client.startCapture()
            } else {
                try await client.stopCapture()
            }
            updateState { $0.capturing = newState }
            return newState
        } catch {
            logger.error("toggleCapture failed: \(error.localizedDescription)")
            return context.capturing
        }
    }

    func sendMessage(_ content: String) async -> String? {
        let userMessage = ChatMessage(
            id: "user-\(Int(Date().timeIntervalSince1970 * 1000))",
            sender: .user,
            content: content,
            isPending: true
        )
        updateState { $0.messages.append(userMessage) }

        do {
            let response = try await client.sendMessage(content)
            return response.messageId
        } catch {
            logger.error("sendMessage failed: \(error.localizedDescription)")
            updateState { ctx in
                ctx.messages = ctx.messages.map {
                    $0.isPending ? ChatMessage(
                        id: $0.id, sender: $0.sender, content: $0.content,
                        timestamp: $0.timestamp, isStreaming: false, isPending: false
                    ) : $0
                }
            }
            return nil
        }
    }

    func dismissMessage() async {
        updateState {
            $0.lastMessage = nil
            $0.currentMessage = ""
            $0.speaking = false
        }
        try? await client.dismissMessage()
    }

    func clearMessages() {
        updateState {
            $0.messages = []
            $0.lastMessage = nil
            $0.currentMessage = ""
            $0.currentMessageId = nil
        }
        streamingMessage = ""
        streamingMessageId = nil
    }

    // MARK: - SSE Event Processing

    private func processBundle(_ bundle: StreamBundle) {
        switch bundle.type {
        case .indicator:
            if let payload = try? bundle.payload.decode(as: IndicatorPayload.self) {
                handleIndicator(payload)
            }
        case .textDelta:
            if let payload = try? bundle.payload.decode(as: TextDeltaPayload.self) {
                handleTextDelta(payload, messageId: bundle.messageId)
            }
        case .toolCall:
            handleToolCall(bundle.payload)
        case .conversationClosed:
            if let payload = try? bundle.payload.decode(as: ConversationClosedPayload.self) {
                handleConversationClosed(payload)
            }
        case .error:
            if let payload = try? bundle.payload.decode(as: ErrorPayload.self) {
                logger.error("Daemon error: \(payload.message)")
            }
        case .heartbeat:
            break
        case .actionRequest:
            break
        }
    }

    private func handleIndicator(_ payload: IndicatorPayload) {
        let indicator = payload.indicator

        // When transitioning to idle with accumulated text, finalize
        if indicator == .idle && !context.currentMessage.isEmpty {
            finalizeStreamingMessage()
            return
        }

        let bubbleIndicators: Set<IndicatorType> = [.thinking, .analyzing]
        let activeIndicator: IndicatorType? = bubbleIndicators.contains(indicator) ? indicator : nil

        updateState { ctx in
            switch indicator {
            case .idle:
                ctx.capturing = false; ctx.thinking = false; ctx.speaking = false
            case .capturing:
                ctx.capturing = true; ctx.thinking = false; ctx.speaking = false
            case .analyzing, .thinking, .generating:
                ctx.thinking = true; ctx.speaking = false
            case .speaking:
                ctx.thinking = false; ctx.speaking = true
            }
            ctx.activeIndicator = activeIndicator
        }
    }

    private func handleTextDelta(_ payload: TextDeltaPayload, messageId: String) {
        if streamingMessageId != messageId {
            streamingMessage = ""
            streamingMessageId = messageId
        }

        streamingMessage += payload.delta

        updateState { ctx in
            // Clear pending state from user messages
            ctx.messages = ctx.messages.map {
                $0.isPending ? ChatMessage(
                    id: $0.id, sender: $0.sender, content: $0.content,
                    timestamp: $0.timestamp, isStreaming: false, isPending: false
                ) : $0
            }

            if let idx = ctx.messages.firstIndex(where: { $0.id == messageId && $0.isStreaming }) {
                ctx.messages[idx].content = self.streamingMessage
            } else {
                ctx.messages.append(ChatMessage(
                    id: messageId, sender: .bobe, content: self.streamingMessage,
                    isStreaming: true
                ))
            }

            ctx.currentMessage = self.streamingMessage
            ctx.currentMessageId = messageId
        }

        if payload.done {
            finalizeStreamingMessage()
        }
    }

    private func finalizeStreamingMessage() {
        guard let msgId = streamingMessageId else { return }

        updateState { ctx in
            ctx.messages = ctx.messages.map {
                $0.id == msgId ? ChatMessage(
                    id: $0.id, sender: $0.sender, content: self.streamingMessage,
                    timestamp: $0.timestamp, isStreaming: false, isPending: false
                ) : $0
            }

            // Trim to max visible
            if ctx.messages.count > maxVisibleMessages {
                ctx.messages = Array(ctx.messages.suffix(maxVisibleMessages))
            }

            ctx.lastMessage = self.streamingMessage
            ctx.currentMessage = ""
            ctx.currentMessageId = nil
            ctx.thinking = false
            ctx.speaking = false
            ctx.activeIndicator = nil
        }

        streamingMessage = ""
        streamingMessageId = nil

        // Auto-clear lastMessage after 30s
        lastMessageTimer?.cancel()
        lastMessageTimer = Task {
            try? await Task.sleep(for: .seconds(30))
            if !Task.isCancelled {
                updateState { $0.lastMessage = nil }
            }
        }
    }

    private func handleToolCall(_ payload: AnyCodablePayload) {
        // Try to decode as start or complete based on status field
        if let start = try? payload.decode(as: ToolCallStartPayload.self), start.status == "start" {
            let execution = ToolExecution(
                toolName: start.toolName,
                toolCallId: start.toolCallId,
                status: .running,
                startedAt: .now
            )
            updateState { $0.toolExecutions.append(execution) }
        } else if let complete = try? payload.decode(as: ToolCallCompletePayload.self), complete.status == "complete" {
            updateState { ctx in
                ctx.toolExecutions = ctx.toolExecutions.map { t in
                    guard t.toolCallId == complete.toolCallId else { return t }
                    var updated = t
                    updated.status = complete.success ? .success : .error
                    updated.error = complete.error
                    updated.durationMs = complete.durationMs
                    updated.completedAt = .now
                    return updated
                }
            }

            // Clean up completed tools after 2s
            let completedId = complete.toolCallId
            Task {
                try? await Task.sleep(for: .seconds(2))
                updateState { ctx in
                    ctx.toolExecutions.removeAll { $0.toolCallId == completedId && $0.status != .running }
                }
            }
        }
    }

    private func handleConversationClosed(_ payload: ConversationClosedPayload) {
        logger.info("Conversation closed: \(payload.conversationId) (\(payload.reason), \(payload.turnCount) turns)")
        clearMessages()
        updateState {
            $0.thinking = false
            $0.speaking = false
            $0.activeIndicator = nil
            $0.toolExecutions = []
        }
    }

    // MARK: - State Update Helper

    private func updateState(_ block: (inout BobeContext) -> Void) {
        var ctx = context
        block(&ctx)
        ctx.stateType = deriveStateType(from: ctx)
        context = ctx
    }
}
