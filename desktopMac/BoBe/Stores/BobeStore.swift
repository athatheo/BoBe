import AppKit
import CoreGraphics
import Foundation
import Observation
import OSLog

private let logger = Logger(subsystem: "com.bobe.app", category: "BobeStore")

@Observable @MainActor
final class BobeStore {
    static let shared = BobeStore()

    // MARK: - State

    private(set) var context = BobeContext()
    private(set) var isReconnecting = false
    private(set) var isBackendFatal = false
    private var hasConnectedOnce = false

    // MARK: - Locale

    var localeOverride: String = ""
    var effectiveLocale: String = "en"
    var supportedLocales: [String] = []
    /// Increments on locale change to force SwiftUI views to re-evaluate L10n.tr() calls.
    private(set) var localeVersion: Int = 0

    var stateType: BobeStateType {
        self.context.stateType
    }

    var isConnected: Bool {
        self.context.daemonConnected
    }

    var isCapturing: Bool {
        self.context.capturing
    }

    var isThinking: Bool {
        self.context.thinking
    }

    var hasMessage: Bool {
        self.context.lastMessage != nil
    }

    var messages: [ChatMessage] {
        self.context.messages
    }

    var errorMessage: String? {
        self.context.errorMessage
    }

    var toolExecutions: [ToolExecution] {
        self.context.toolExecutions
    }

    var runningTools: [ToolExecution] {
        self.context.toolExecutions.filter { $0.status == .running }
    }

    var capturePermissionMissing: Bool {
        self.context.capturePermissionMissing
    }

    // MARK: - Private

    private let client = DaemonClient.shared
    private var streamingMessage = ""
    private var streamingMessageId: String?
    private var lastMessageTimer: Task<Void, Never>?
    private var textDeltaFlushTask: Task<Void, Never>?
    private var captureStartupTask: Task<Void, Never>?
    private var appNapActivity: NSObjectProtocol?
    private var backendObserverTask: Task<Void, Never>?
    private var sleepWakeObservers: [NSObjectProtocol] = []
    @ObservationIgnored
    private lazy var toolExecutionController = ToolExecutionController { [weak self] mutation in
        self?.updateState(mutation)
    }

    private init() {}

    func observeBackendState() {
        guard self.backendObserverTask == nil else { return }
        self.backendObserverTask = Task { [weak self] in
            for await state in BackendService.shared.stateStream {
                guard !Task.isCancelled else { return }
                await MainActor.run { [weak self] in
                    switch state {
                    case .fatal:
                        self?.isBackendFatal = true
                        self?.isReconnecting = false
                        self?.updateState { ctx in
                            ctx.daemonConnected = false
                            ctx.daemonError = true
                        }
                    case .ready:
                        self?.isBackendFatal = false
                        self?.isReconnecting = false
                        self?.updateState { $0.daemonError = false }
                    case .crashed:
                        self?.isReconnecting = true
                    default:
                        break
                    }
                }
            }
        }
    }

    /// Reconnects SSE on wake since the TCP connection is likely stale.
    private func registerSleepWakeObservers() {
        guard self.sleepWakeObservers.isEmpty else { return }
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
        self.sleepWakeObservers.append(wakeObserver)
    }

    // MARK: - Connection

    func connect() {
        self.observeBackendState()
        self.registerSleepWakeObservers()
        if self.appNapActivity == nil {
            self.appNapActivity = ProcessInfo.processInfo.beginActivity(
                options: .userInitiated,
                reason: "Maintaining SSE connection to backend"
            )
        }
        Task {
            await self.client.connectSSE(
                onEvent: { [weak self] bundle in
                    Task { @MainActor in
                        self?.processBundle(bundle)
                    }
                },
                onConnectionChange: { [weak self] connected in
                    Task { @MainActor in
                        self?.updateState { ctx in
                            ctx.daemonConnected = connected
                            if connected { ctx.daemonError = false }
                        }
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
        self.textDeltaFlushTask?.cancel()
        self.lastMessageTimer?.cancel()
        self.captureStartupTask?.cancel()
        self.backendObserverTask?.cancel()
        for observer in self.sleepWakeObservers {
            NSWorkspace.shared.notificationCenter.removeObserver(observer)
        }
        self.sleepWakeObservers.removeAll()
        if let activity = appNapActivity {
            ProcessInfo.processInfo.endActivity(activity)
            self.appNapActivity = nil
        }
        Task {
            await self.client.disconnectSSE()
        }
    }

    func beginShutdown() {
        self.updateState { $0.shuttingDown = true }
    }

    // MARK: - Locale

    func updateLocale(_ newOverride: String) {
        self.localeOverride = newOverride
        self.localeVersion += 1
        L10n.setLocaleOverride(newOverride.isEmpty ? nil : newOverride)
    }

    // MARK: - Actions

    func dismissError() {
        self.updateState { ctx in
            ctx.errorMessage = nil
            ctx.daemonError = false
        }
    }

    func toggleCapture() async -> Bool {
        let newState = !self.context.capturing
        do {
            if newState {
                try await self.client.startCapture()
            } else {
                try await self.client.stopCapture()
            }
            self.updateState { ctx in
                ctx.capturing = newState
                if !newState {
                    ctx.captureInProgress = false
                }
            }
            return newState
        } catch {
            logger.error("toggleCapture failed: \(error.localizedDescription)")
            if newState, !CGPreflightScreenCaptureAccess() {
                CGRequestScreenCaptureAccess()
                self.updateState { $0.capturePermissionMissing = true }
            }
            return self.context.capturing
        }
    }

    func sendMessage(_ content: String) async -> String? {
        let userMessage = ChatMessage(
            id: "user-\(Int(Date().timeIntervalSince1970 * 1000))",
            sender: .user,
            content: content,
            isPending: true
        )
        self.updateState { $0.messages.append(userMessage) }

        do {
            let response = try await client.sendMessage(content)
            return response.messageId
        } catch {
            logger.error("sendMessage failed: \(error.localizedDescription)")
            self.updateState { ctx in
                Self.normalizePendingMessages(&ctx.messages)
            }
            return nil
        }
    }

    func clearMessages() {
        self.updateState {
            $0.messages = []
            $0.lastMessage = nil
            $0.currentMessage = ""
            $0.currentMessageId = nil
        }
        self.streamingMessage = ""
        self.streamingMessageId = nil
    }

    // MARK: - SSE Event Processing

    private func processBundle(_ bundle: StreamBundle) {
        switch bundle.type {
        case .indicator:
            if let payload = try? bundle.payload.decode(as: IndicatorPayload.self) {
                self.handleIndicator(payload)
            }
        case .textDelta:
            if let payload = try? bundle.payload.decode(as: TextDeltaPayload.self) {
                self.handleTextDelta(payload, messageId: bundle.messageId)
            }
        case .toolCall, .toolCallStart:
            self.handleToolCall(bundle.payload)
        case .toolCallComplete:
            self.handleToolCall(bundle.payload)
        case .conversationClosed:
            if let payload = try? bundle.payload.decode(as: ConversationClosedPayload.self) {
                self.handleConversationClosed(payload)
            }
        case .error:
            if let payload = try? bundle.payload.decode(as: ErrorPayload.self) {
                logger.error("Daemon error: \(payload.message)")
                if !payload.recoverable {
                    self.updateState { ctx in
                        ctx.errorMessage = payload.message
                        ctx.daemonError = true
                    }
                }
            }
        case .endOfTurn:
            self.finalizeStreamingMessage()
        case .heartbeat, .unknown:
            break
        }
    }

    private func handleIndicator(_ payload: IndicatorPayload) {
        let indicator = payload.indicator

        if indicator == .idle, !self.context.currentMessage.isEmpty {
            self.finalizeStreamingMessage()
            return
        }

        let bubbleIndicators: Set<IndicatorType> = [.thinking, .toolCalling]
        let activeIndicator: IndicatorType? = bubbleIndicators.contains(indicator) ? indicator : nil

        self.updateState { ctx in
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
            if indicator != .unknown {
                ctx.errorMessage = nil
                ctx.daemonError = false
            }
        }
    }

    private func handleTextDelta(_ payload: TextDeltaPayload, messageId: String) {
        if self.streamingMessageId != messageId {
            self.streamingMessage = ""
            self.streamingMessageId = messageId
            self.textDeltaFlushTask?.cancel()
            self.textDeltaFlushTask = nil
        }

        self.streamingMessage += payload.delta

        if payload.done {
            self.flushStreamingToUI(messageId: messageId)
            self.finalizeStreamingMessage()
            return
        }

        // Throttle UI updates: flush at ~150ms intervals for a typing appearance.
        // Task existence is the dirty flag — if a task is already scheduled, new
        // deltas just accumulate in streamingMessage until the timer fires.
        if self.textDeltaFlushTask == nil {
            self.textDeltaFlushTask = Task { @MainActor [weak self] in
                try? await Task.sleep(for: .milliseconds(StoreTiming.textDeltaFlushMilliseconds))
                guard let self, !Task.isCancelled else { return }
                self.flushStreamingToUI(messageId: messageId)
            }
        }
    }

    private func flushStreamingToUI(messageId: String) {
        self.textDeltaFlushTask?.cancel()
        self.textDeltaFlushTask = nil

        self.updateState { ctx in
            Self.normalizePendingMessages(&ctx.messages)

            if let idx = ctx.messages.firstIndex(where: { $0.id == messageId && $0.isStreaming }) {
                ctx.messages[idx].content = self.streamingMessage
            } else {
                ctx.messages.append(
                    ChatMessage(
                        id: messageId, sender: .bobe, content: self.streamingMessage,
                        isStreaming: true
                    )
                )
            }

            ctx.currentMessage = self.streamingMessage
            ctx.currentMessageId = messageId
            let hasVisibleText = self.hasVisibleGlyphs(self.streamingMessage)
            ctx.thinking = !hasVisibleText
            ctx.speaking = hasVisibleText
        }
    }

    private func finalizeStreamingMessage() {
        self.textDeltaFlushTask?.cancel()
        self.textDeltaFlushTask = nil

        guard let msgId = streamingMessageId else { return }

        self.updateState { ctx in
            ctx.messages = ctx.messages.map {
                $0.id == msgId
                    ? ChatMessage(
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

        self.streamingMessage = ""
        self.streamingMessageId = nil

        self.lastMessageTimer?.cancel()
        self.lastMessageTimer = Task { [weak self] in
            try? await Task.sleep(for: .seconds(StoreTiming.lastMessageClearSeconds))
            if !Task.isCancelled {
                self?.updateState { $0.lastMessage = nil }
            }
        }
    }

    private func handleToolCall(_ payload: AnyCodablePayload) {
        self.toolExecutionController.process(payload)
    }

    private func handleConversationClosed(_ payload: ConversationClosedPayload) {
        logger.info("Conversation closed: \(payload.conversationId) (\(payload.reason), \(payload.turnCount) turns)")
        self.updateState {
            $0.thinking = false
            $0.speaking = false
            $0.activeIndicator = nil
            $0.toolExecutions = []
        }
        Task {
            try? await Task.sleep(for: .seconds(StoreTiming.conversationClearSeconds))
            self.clearMessages()
        }
    }

    private func synchronizeCaptureStartup() {
        self.captureStartupTask?.cancel()
        self.captureStartupTask = Task { @MainActor [weak self] in
            guard let self else { return }
            do {
                let settings = try await client.getSettings()

                // Apply locale from daemon settings
                self.effectiveLocale = settings.effectiveLocale
                self.supportedLocales = settings.supportedLocales
                let override = settings.localeOverride ?? ""
                self.localeOverride = override
                L10n.setLocaleOverride(override.isEmpty ? nil : override)

                guard settings.captureEnabled else {
                    self.updateState { ctx in
                        ctx.capturing = false
                        ctx.captureInProgress = false
                    }
                    return
                }

                var captureActive = false
                for attempt in 0 ..< 3 where !Task.isCancelled {
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
                            try? await Task.sleep(for: .milliseconds(StoreTiming.captureRetryBaseMilliseconds * (attempt + 1)))
                        }
                    }
                }

                if !captureActive, !CGPreflightScreenCaptureAccess() {
                    CGRequestScreenCaptureAccess()
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

    private static func normalizePendingMessages(_ messages: inout [ChatMessage]) {
        messages = messages.map { message in
            guard message.isPending else { return message }
            return ChatMessage(
                id: message.id,
                sender: message.sender,
                content: message.content,
                timestamp: message.timestamp,
                isStreaming: false,
                isPending: false
            )
        }
    }

    private func updateState(_ block: (inout BobeContext) -> Void) {
        var ctx = self.context
        block(&ctx)
        ctx.stateType = deriveStateType(from: ctx)
        self.context = ctx
    }
}
