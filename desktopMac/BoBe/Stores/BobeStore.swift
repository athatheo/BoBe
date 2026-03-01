import AppKit
import CoreGraphics
import Foundation
import Observation
import OSLog

private let logger = Logger(subsystem: "com.bobe.app", category: "BobeStore")

/// Central observable state store for the BoBe app.
/// Replaces the original useSyncExternalStore + bobeActions pattern.
@Observable @MainActor
final class BobeStore {
    static let shared = BobeStore()

    // MARK: - State

    private(set) var context = BobeContext()
    private(set) var isReconnecting = false
    /// True when the backend has crashed beyond recovery (3+ restarts).
    private(set) var isBackendFatal = false

    /// Set after initial SSE connection is established
    private var hasConnectedOnce = false

    var stateType: BobeStateType { context.stateType }
    var isConnected: Bool { context.daemonConnected }
    var isCapturing: Bool { context.capturing }
    var isThinking: Bool { context.thinking }
    var hasMessage: Bool { context.lastMessage != nil }
    var messages: [ChatMessage] { context.messages }
    var errorMessage: String? { context.errorMessage }
    var toolExecutions: [ToolExecution] { context.toolExecutions }
    var runningTools: [ToolExecution] { context.toolExecutions.filter { $0.status == .running } }
    var capturePermissionMissing: Bool { context.capturePermissionMissing }

    // MARK: - Private

    private let client = DaemonClient.shared
    private var streamingMessage = ""
    private var streamingMessageId: String?
    private var lastMessageTimer: Task<Void, Never>?
    private var captureStartupTask: Task<Void, Never>?
    /// Prevents App Nap from throttling the SSE connection.
    private var appNapActivity: NSObjectProtocol?
    private var backendObserverTask: Task<Void, Never>?
    private var sleepWakeObservers: [NSObjectProtocol] = []

    private init() {}

    /// Observe backend service state and update UI accordingly.
    /// Called once — persists across reconnects. The sleep/wake handler
    /// reconnects SSE without recreating this observer.
    func observeBackendState() {
        guard backendObserverTask == nil else { return }
        backendObserverTask = Task { [weak self] in
            for await state in BackendService.shared.stateStream {
                guard !Task.isCancelled else { return }
                await MainActor.run { [weak self] in
                    switch state {
                    case .fatal:
                        self?.isBackendFatal = true
                        self?.isReconnecting = false
                        self?.updateState { $0.daemonConnected = false }
                    case .ready:
                        self?.isBackendFatal = false
                        self?.isReconnecting = false
                    case .crashed:
                        self?.isReconnecting = true
                    default:
                        break
                    }
                }
            }
        }
    }

    /// Proactively reconnect SSE on wake — the TCP connection is likely stale.
    private func registerSleepWakeObservers() {
        guard sleepWakeObservers.isEmpty else { return }
        let center = NSWorkspace.shared.notificationCenter
        let wakeObserver = center.addObserver(
            forName: NSWorkspace.didWakeNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor [weak self] in
                guard let self, !self.isBackendFatal else { return }
                logger.info("System woke — reconnecting SSE")
                await self.client.disconnectSSE()
                self.connect()
            }
        }
        sleepWakeObservers.append(wakeObserver)
    }

    // MARK: - Connection

    func connect() {
        observeBackendState()
        registerSleepWakeObservers()
        if appNapActivity == nil {
            appNapActivity = ProcessInfo.processInfo.beginActivity(
                options: .userInitiated,
                reason: "Maintaining SSE connection to backend"
            )
        }
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
                            self?.isReconnecting = false
                            self?.hasConnectedOnce = true
                            self?.synchronizeCaptureStartup()
                        } else if self?.hasConnectedOnce == true {
                            self?.isReconnecting = true
                        }
                    }
                }
            )
        }
    }

    func disconnect() {
        captureStartupTask?.cancel()
        backendObserverTask?.cancel()
        for observer in sleepWakeObservers {
            NSWorkspace.shared.notificationCenter.removeObserver(observer)
        }
        sleepWakeObservers.removeAll()
        if let activity = appNapActivity {
            ProcessInfo.processInfo.endActivity(activity)
            appNapActivity = nil
        }
        Task {
            await client.disconnectSSE()
        }
    }

    func beginShutdown() {
        updateState { $0.shuttingDown = true }
    }

    // MARK: - Actions

    func dismissError() {
        updateState { $0.errorMessage = nil }
    }

    func toggleCapture() async -> Bool {
        let newState = !context.capturing
        do {
            if newState {
                try await client.startCapture()
            } else {
                try await client.stopCapture()
            }
            updateState { ctx in
                ctx.capturing = newState
                if !newState {
                    ctx.captureInProgress = false
                }
            }
            return newState
        } catch {
            logger.error("toggleCapture failed: \(error.localizedDescription)")
            // If starting capture failed, check if screen recording permission was revoked.
            if newState && !CGPreflightScreenCaptureAccess() {
                updateState { $0.capturePermissionMissing = true }
            }
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
        case .toolCall, .toolCallStart:
            handleToolCall(bundle.payload)
        case .toolCallComplete:
            handleToolCall(bundle.payload)
        case .conversationClosed:
            if let payload = try? bundle.payload.decode(as: ConversationClosedPayload.self) {
                handleConversationClosed(payload)
            }
        case .error:
            if let payload = try? bundle.payload.decode(as: ErrorPayload.self) {
                logger.error("Daemon error: \(payload.message)")
                if !payload.recoverable {
                    updateState { $0.errorMessage = payload.message }
                }
            }
        case .endOfTurn:
            finalizeStreamingMessage()
        case .heartbeat, .unknown:
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

        let bubbleIndicators: Set<IndicatorType> = [.thinking, .toolCalling]
        let activeIndicator: IndicatorType? = bubbleIndicators.contains(indicator) ? indicator : nil

        updateState { ctx in
            switch indicator {
            case .idle:
                ctx.captureInProgress = false
                ctx.thinking = false
                ctx.speaking = false
            case .screenCapture:
                ctx.captureInProgress = true
                ctx.thinking = false
                ctx.speaking = false
            case .toolCalling, .thinking:
                ctx.captureInProgress = false
                ctx.thinking = true
                ctx.speaking = false
            case .streaming:
                ctx.captureInProgress = false
                let hasVisibleText = self.hasVisibleGlyphs(self.streamingMessage) || self.hasVisibleGlyphs(ctx.currentMessage)
                ctx.thinking = !hasVisibleText
                ctx.speaking = hasVisibleText
            case .unknown:
                break
            }
            ctx.activeIndicator = activeIndicator
            // Clear any error when backend resumes normal operation
            if indicator != .unknown { ctx.errorMessage = nil }
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
            let hasVisibleText = self.hasVisibleGlyphs(self.streamingMessage)
            ctx.thinking = !hasVisibleText
            ctx.speaking = hasVisibleText
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
                try? await Task.sleep(for: .seconds(5))
                updateState { ctx in
                    ctx.toolExecutions.removeAll { $0.toolCallId == completedId && $0.status != .running }
                }
            }
        }
    }

    private func handleConversationClosed(_ payload: ConversationClosedPayload) {
        logger.info("Conversation closed: \(payload.conversationId) (\(payload.reason), \(payload.turnCount) turns)")
        updateState {
            $0.thinking = false
            $0.speaking = false
            $0.activeIndicator = nil
            $0.toolExecutions = []
        }
        // Delay message clear so user sees the conversation end naturally
        Task {
            try? await Task.sleep(for: .seconds(3))
            clearMessages()
        }
    }

    private func synchronizeCaptureStartup() {
        captureStartupTask?.cancel()
        captureStartupTask = Task { @MainActor [weak self] in
            guard let self else { return }
            do {
                let settings = try await self.client.getSettings()
                guard settings.captureEnabled else {
                    self.updateState { ctx in
                        ctx.capturing = false
                        ctx.captureInProgress = false
                    }
                    return
                }

                var captureActive = false
                for attempt in 0..<3 where !Task.isCancelled {
                    do {
                        try await self.client.startCapture()
                        captureActive = true
                        break
                    } catch let DaemonError.httpError(statusCode, message)
                        where statusCode == 409 || message.localizedCaseInsensitiveContains("already") {
                        captureActive = true
                        break
                    } catch {
                        logger.warning("Capture startup sync attempt \(attempt + 1) failed: \(error.localizedDescription)")
                        if attempt < 2 {
                            try? await Task.sleep(for: .milliseconds(350 * (attempt + 1)))
                        }
                    }
                }

                if !captureActive && !CGPreflightScreenCaptureAccess() {
                    self.updateState { $0.capturePermissionMissing = true }
                    logger.warning("Screen capture permission not granted")
                } else {
                    self.updateState { $0.capturePermissionMissing = false }
                }
                self.updateState { ctx in
                    ctx.capturing = captureActive
                    ctx.captureInProgress = false
                }
            } catch {
                logger.warning("Capture startup sync skipped: \(error.localizedDescription)")
            }
        }
    }

    private func hasVisibleGlyphs(_ text: String) -> Bool {
        text.unicodeScalars.contains(where: {
            !$0.properties.isWhitespace && !CharacterSet.controlCharacters.contains($0)
        })
    }

    // MARK: - State Update Helper

    private func updateState(_ block: (inout BobeContext) -> Void) {
        var ctx = context
        block(&ctx)
        ctx.stateType = deriveStateType(from: ctx)
        context = ctx
    }
}
