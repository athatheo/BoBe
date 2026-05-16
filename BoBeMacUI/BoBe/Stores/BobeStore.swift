import AppKit
import CoreGraphics
import Foundation
import Observation
import OSLog

let bobeStoreLogger = Logger(subsystem: "com.bobe.app", category: "BobeStore")

@Observable @MainActor
final class BobeStore {
    static let shared = BobeStore()

    // MARK: - State

    private(set) var context = BobeContext()
    var isReconnecting = false
    var isBackendFatal = false
    var hasConnectedOnce = false

    // MARK: - Locale (client-side only)

    /// Empty string means "follow system locale".
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
        // Daemon flag covers SSE-vs-daemon race; without this, rapid double-sends 409.
        if !self.context.acceptingUserMessages {
            return .thinking
        }
        return nil
    }

    // MARK: - Private storage shared with extensions

    let client = DaemonClient.shared
    var streamingMessage = ""
    var streamingMessageId: String?
    var lastMessageTimer: Task<Void, Never>?
    var conversationClearTask: Task<Void, Never>?
    var textDeltaFlushTask: Task<Void, Never>?
    var captureStartupTask: Task<Void, Never>?
    private var appNapActivity: NSObjectProtocol?
    private var backendObserverTask: Task<Void, Never>?
    private var sleepWakeObservers: [NSObjectProtocol] = []
    @ObservationIgnored
    var reconnectStatusTask: Task<Void, Never>?
    @ObservationIgnored
    lazy var toolExecutionController = ToolExecutionController { [weak self] mutation in
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
                bobeStoreLogger.info("System woke — reconnecting SSE")
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
            bobeStoreLogger.error("toggleCapture failed: \(error.localizedDescription)")
            if newState, !CGPreflightScreenCaptureAccess() {
                CGRequestScreenCaptureAccess()
                self.updateState { $0.capturePermissionMissing = true }
            }
            return self.context.capturing
        }
    }

    /// Voice variant — the daemon has already persisted the user turn server-side
    /// (via `handle_user_message_with_observer`), so we just mirror it into the
    /// overlay chat history without firing another HTTP send.
    func appendUserVoiceMessage(_ content: String) {
        self.cancelConversationClear()
        let userMessage = ChatMessage(
            id: "voice-\(Int(Date().timeIntervalSince1970 * 1000))",
            sender: .user,
            content: content,
            isPending: false
        )
        self.updateState { ctx in
            ctx.errorMessage = nil
            ctx.conversationEnding = false
            ctx.messages.append(userMessage)
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
                // Optimistic lock — closes SSE-vs-daemon indicator race.
                ctx.acceptingUserMessages = false
            }
        } catch {
            // 409 = daemon busy; retry banner suffices, skip red banner.
            let isBusy409 = Self.isBusy409(error)
            bobeStoreLogger.error("sendMessage failed: \(error.localizedDescription)")
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

    static func markMessageSent(_ messageId: String, messages: inout [ChatMessage]) {
        Self.updateMessage(messageId, messages: &messages) { message in
            message.isStreaming = false
            message.isPending = false
        }
    }

    static func removeMessage(_ messageId: String, messages: inout [ChatMessage]) {
        messages.removeAll { $0.id == messageId }
    }

    @discardableResult
    static func updateMessage(
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

    func updateState(_ block: (inout BobeContext) -> Void) {
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
