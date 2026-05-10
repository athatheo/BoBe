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

    // MARK: - Locale (client-side only)
    //
    // Locale is purely a SwiftUI concern post-Copilot-SDK pivot — the
    // daemon doesn't translate anything. Persist to UserDefaults so
    // the choice survives restarts; empty string means "follow system".

    private static let localeOverrideKey = "bobe.locale_override"
    static let supportedLocales = [
        "en-US", "el-GR", "zh-CN", "de-DE", "es-ES", "pt-BR", "ko-KR", "ja-JP", "fr-FR",
    ]
    private(set) var localeOverride: String =
        UserDefaults.standard.string(forKey: BobeStore.localeOverrideKey) ?? ""

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

    var failedSendRecoveries: [FailedSendRecovery] {
        self.context.failedSendRecoveries
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

    var softWarning: String? {
        self.context.softWarning
    }

    var indicatorMessage: String? {
        self.context.indicatorMessage
    }

    var conversationEnding: Bool {
        self.context.conversationEnding
    }

    var canSendMessage: Bool {
        self.context.daemonConnected && self.context.acceptingUserMessages
    }

    var isInitialConnectionPending: Bool {
        !self.hasConnectedOnce && !self.context.daemonConnected && !self.isBackendFatal
    }

    var composerBlockReason: MessageComposerBlockReason? {
        if !self.context.daemonConnected {
            return self.hasConnectedOnce ? .reconnecting : .starting
        }
        if let tool = self.runningTools.first {
            return .usingTool(tool.toolName)
        }
        if self.context.speaking {
            return .speaking
        }
        if self.context.thinking {
            return .thinking
        }
        if self.context.captureInProgress {
            return .capturing
        }
        // Daemon owns the authoritative "accepting" flag — covers the
        // gap between try_begin_user_message setting it false and the
        // indicator transitioning. Without this, rapid double-sends 409.
        if !self.context.acceptingUserMessages {
            return .thinking
        }
        return nil
    }

    // MARK: - Private

    private let client = DaemonClient.shared
    private var streamingMessage = ""
    private var streamingMessageId: String?
    private var lastMessageTimer: Task<Void, Never>?
    private var conversationClearTask: Task<Void, Never>?
    private var textDeltaFlushTask: Task<Void, Never>?
    private var captureStartupTask: Task<Void, Never>?
    private var appNapActivity: NSObjectProtocol?
    private var backendObserverTask: Task<Void, Never>?
    private var sleepWakeObservers: [NSObjectProtocol] = []
    @ObservationIgnored
    private var reconnectStatusTask: Task<Void, Never>?
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
                        self?.cancelReconnectStatusTransition()
                        self?.isBackendFatal = true
                        self?.isReconnecting = false
                        self?.updateState { ctx in
                            ctx.daemonConnected = false
                            ctx.daemonError = true
                        }
                    case .ready:
                        self?.cancelReconnectStatusTransition()
                        self?.isBackendFatal = false
                        self?.isReconnecting = false
                        self?.updateState { $0.daemonError = false }
                    case .crashed:
                        self?.scheduleReconnectStatusTransition()
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
                        self?.handleConnectionChange(connected)
                    }
                }
            )
        }
    }

    func disconnect() {
        self.textDeltaFlushTask?.cancel()
        self.lastMessageTimer?.cancel()
        self.conversationClearTask?.cancel()
        self.captureStartupTask?.cancel()
        self.backendObserverTask?.cancel()
        self.reconnectStatusTask?.cancel()
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

    func updateLocale(_ newOverride: String) {
        self.localeOverride = newOverride
        if newOverride.isEmpty {
            UserDefaults.standard.removeObject(forKey: BobeStore.localeOverrideKey)
        } else {
            UserDefaults.standard.set(newOverride, forKey: BobeStore.localeOverrideKey)
        }
        L10n.setLocaleOverride(newOverride.isEmpty ? nil : newOverride)
    }

    /// Apply persisted override on app launch.
    func applyPersistedLocale() {
        L10n.setLocaleOverride(self.localeOverride.isEmpty ? nil : self.localeOverride)
    }

    // MARK: - Actions

    func dismissError() {
        self.updateState { ctx in
            ctx.errorMessage = nil
            ctx.daemonError = false
        }
    }

    func dismissSoftWarning() {
        self.updateState { $0.softWarning = nil }
    }

    /// Push a non-fatal warning (e.g. backend degraded subsystem) through
    /// the same banner channel as transient errors. The user can dismiss
    /// it via the X button on the overlay error banner.
    func surfaceWarning(_ message: String) {
        self.updateState { ctx in
            ctx.errorMessage = message
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

    func sendMessage(_ content: String) async {
        self.cancelConversationClear()
        let userMessage = ChatMessage(
            id: "user-\(Int(Date().timeIntervalSince1970 * 1000))",
            sender: .user,
            content: content,
            isPending: true
        )
        self.updateState { ctx in
            ctx.errorMessage = nil
            ctx.conversationEnding = false
            ctx.messages.append(userMessage)
        }

        do {
            try await client.sendMessage(content)
            self.updateState { ctx in
                Self.markMessageSent(userMessage.id, messages: &ctx.messages)
                // Optimistically lock the composer until the indicator
                // catches up — closes the SSE-vs-daemon race window.
                ctx.acceptingUserMessages = false
            }
        } catch {
            // Daemon returns 409 when busy with one of 4 hardcoded
            // "BoBe is still..." strings. The retry banner is the right
            // affordance — don't double up with a fatal-looking red
            // error banner. For other failures (network, etc.) we DO
            // surface errorMessage so the user knows it wasn't busy.
            let isBusy409 = Self.isBusy409(error)
            logger.error("sendMessage failed: \(error.localizedDescription)")
            self.updateState { ctx in
                Self.removeMessage(userMessage.id, messages: &ctx.messages)
                ctx.failedSendRecoveries.append(
                    FailedSendRecovery(id: userMessage.id, content: content)
                )
                if !isBusy409 {
                    ctx.errorMessage = error.localizedDescription
                    ctx.daemonError = false
                }
                ctx.acceptingUserMessages = !isBusy409
            }
        }
    }

    private static func isBusy409(_ error: any Error) -> Bool {
        if case let DaemonError.httpError(statusCode, _) = error {
            return statusCode == 409
        }
        return false
    }

    func dismissFailedSendRecovery(_ recoveryId: String) {
        self.updateState { ctx in
            ctx.failedSendRecoveries.removeAll { $0.id == recoveryId }
        }
    }

    func retryFailedSendRecovery(_ recoveryId: String) async {
        guard let recovery = self.context.failedSendRecoveries.first(where: { $0.id == recoveryId }) else {
            return
        }

        self.updateState { ctx in
            ctx.failedSendRecoveries.removeAll { $0.id == recoveryId }
        }

        await self.sendMessage(recovery.content)
    }

    func clearMessages() {
        self.cancelConversationClear()
        self.updateState {
            $0.messages = []
            $0.lastMessage = nil
            $0.currentMessage = ""
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
            // The daemon emits a `text_delta` with `done: true` as the
            // end-of-turn marker; `handleTextDelta` calls
            // `finalizeStreamingMessage` when it sees that flag.
            if let payload = try? bundle.payload.decode(as: TextDeltaPayload.self) {
                self.handleTextDelta(payload, messageId: bundle.messageId)
            }
        case .toolCallStart, .toolCallComplete:
            self.handleToolCall(bundle.payload)
        case .conversationClosed:
            if let payload = try? bundle.payload.decode(as: ConversationClosedPayload.self) {
                self.handleConversationClosed(payload)
            }
        case .error:
            if let payload = try? bundle.payload.decode(as: ErrorPayload.self) {
                self.handleErrorPayload(payload)
            }
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

        let activeIndicator: IndicatorType? = (indicator == .thinking) ? .thinking : nil

        self.updateState { ctx in
            switch indicator {
            case .idle:
                ctx.captureInProgress = false
                ctx.thinking = false
                ctx.speaking = false
                ctx.acceptingUserMessages = true
            case .screenCapture:
                ctx.captureInProgress = true
                ctx.thinking = false
                ctx.speaking = false
                ctx.acceptingUserMessages = false
            case .thinking:
                ctx.captureInProgress = false
                ctx.thinking = true
                ctx.speaking = false
                ctx.acceptingUserMessages = false
            case .streaming:
                ctx.captureInProgress = false
                let hasVisibleText = self.hasVisibleGlyphs(self.streamingMessage) || self.hasVisibleGlyphs(ctx.currentMessage)
                ctx.thinking = !hasVisibleText
                ctx.speaking = hasVisibleText
                ctx.acceptingUserMessages = false
            case .unknown:
                break
            }
            ctx.activeIndicator = activeIndicator
            ctx.indicatorMessage = payload.message
            if indicator != .unknown {
                ctx.errorMessage = nil
                ctx.daemonError = false
            }
        }
    }

    /// Decode either chat-stream errors (`code` discriminator) or
    /// background trigger errors (`trigger` discriminator). Recoverable
    /// trigger errors land in the soft-warning banner; recoverable chat
    /// errors do nothing (the stream may continue); fatal errors stop
    /// the world via the red banner.
    private func handleErrorPayload(_ payload: ErrorPayload) {
        if payload.recoverable {
            if payload.isTriggerError {
                logger.warning("Trigger soft warning [\(payload.sourceLabel)]: \(payload.message)")
                self.updateState { $0.softWarning = payload.message }
            } else {
                logger.warning("Recoverable chat error [\(payload.sourceLabel)]: \(payload.message)")
            }
            return
        }
        logger.error("Daemon error [\(payload.sourceLabel)]: \(payload.message)")
        self.updateState { ctx in
            ctx.errorMessage = payload.message
            ctx.daemonError = true
        }
    }

    private func handleTextDelta(_ payload: TextDeltaPayload, messageId: String) {
        self.cancelConversationClear()
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
            let updated = Self.updateMessage(messageId, messages: &ctx.messages) { message in
                message.content = self.streamingMessage
                message.isStreaming = true
            }
            if !updated {
                ctx.messages.append(
                    ChatMessage(
                        id: messageId, sender: .bobe, content: self.streamingMessage,
                        isStreaming: true
                    )
                )
            }

            ctx.currentMessage = self.streamingMessage
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
            Self.updateMessage(msgId, messages: &ctx.messages) { message in
                message.content = self.streamingMessage
                message.isStreaming = false
                message.isPending = false
            }

            ctx.lastMessage = self.streamingMessage
            ctx.currentMessage = ""
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
            $0.conversationEnding = true
        }
        self.scheduleConversationClear()
    }

    private func synchronizeCaptureStartup() {
        self.captureStartupTask?.cancel()
        self.captureStartupTask = Task { @MainActor [weak self] in
            guard let self else { return }
            do {
                let settings = try await client.getSettings()

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

    private func handleConnectionChange(_ connected: Bool) {
        self.updateState { ctx in
            ctx.daemonConnected = connected
            if connected { ctx.daemonError = false }
        }

        if connected {
            self.cancelReconnectStatusTransition()
            self.isReconnecting = false
            self.hasConnectedOnce = true
            self.synchronizeCaptureStartup()
            self.synchronizeStatus()
            return
        }

        guard self.hasConnectedOnce else { return }
        self.scheduleReconnectStatusTransition()
    }

    /// Seed `acceptingUserMessages`, `capturing`, and the indicator state
    /// from the daemon's authoritative `/status` snapshot. Without this,
    /// SSE-only state can drift on mid-turn reconnects — the composer
    /// re-enables before the daemon is actually ready and the first send
    /// 409s.
    private func synchronizeStatus() {
        Task { @MainActor [weak self] in
            guard let self else { return }
            do {
                let status = try await self.client.getStatus()
                self.updateState { ctx in
                    ctx.acceptingUserMessages = status.acceptingUserMessages
                    let indicator = status.indicatorType
                    ctx.thinking = indicator == .thinking
                    ctx.speaking = indicator == .streaming
                    ctx.captureInProgress = indicator == .screenCapture
                }
            } catch {
                logger.warning("Status sync skipped: \(error.localizedDescription)")
            }
        }
    }

    private func scheduleReconnectStatusTransition() {
        self.reconnectStatusTask?.cancel()
        self.reconnectStatusTask = Task { @MainActor [weak self] in
            try? await Task.sleep(for: .milliseconds(StoreTiming.reconnectStatusDelayMilliseconds))
            guard let self, !Task.isCancelled, !self.context.daemonConnected else { return }
            self.isReconnecting = true
        }
    }

    private func cancelReconnectStatusTransition() {
        self.reconnectStatusTask?.cancel()
        self.reconnectStatusTask = nil
    }

    private func hasVisibleGlyphs(_ text: String) -> Bool {
        text.unicodeScalars.contains(where: {
            !$0.properties.isWhitespace && !CharacterSet.controlCharacters.contains($0)
        })
    }

    private func scheduleConversationClear() {
        self.cancelConversationClear()
        self.conversationClearTask = Task { @MainActor [weak self] in
            try? await Task.sleep(for: .seconds(StoreTiming.conversationClearSeconds))
            guard let self, !Task.isCancelled else { return }
            self.clearMessages()
            self.updateState { $0.conversationEnding = false }
        }
    }

    private func cancelConversationClear() {
        self.conversationClearTask?.cancel()
        self.conversationClearTask = nil
    }

    private static func markMessageSent(_ messageId: String, messages: inout [ChatMessage]) {
        Self.updateMessage(messageId, messages: &messages) { message in
            message.isStreaming = false
            message.isPending = false
        }
    }

    private static func removeMessage(_ messageId: String, messages: inout [ChatMessage]) {
        messages.removeAll { $0.id == messageId }
    }

    @discardableResult
    private static func updateMessage(
        _ messageId: String,
        messages: inout [ChatMessage],
        mutate: (inout ChatMessage) -> Void
    ) -> Bool {
        guard let idx = messages.firstIndex(where: { $0.id == messageId }) else {
            return false
        }

        mutate(&messages[idx])
        return true
    }

    private func updateState(_ block: (inout BobeContext) -> Void) {
        let oldCapturing = self.context.capturing
        var ctx = self.context
        block(&ctx)
        ctx.stateType = deriveStateType(from: ctx)
        self.context = ctx
        if ctx.capturing != oldCapturing {
            NotificationCenter.default.post(name: .bobeCaptureStateChanged, object: nil)
        }
    }
}
