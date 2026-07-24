import AppKit
import CoreGraphics
import Foundation
import Observation
import OSLog

let bobeStoreLogger = Logger(subsystem: "com.bobe.app", category: "BobeStore")

@Observable @MainActor
final class BobeStore {
    static let shared = BobeStore()
    static let messageWindowCapacity = 200

    // MARK: - State

    private(set) var context = BobeContext()
    var isReconnecting = false
    var isBackendFatal = false
    var hasConnectedOnce = false

    /// User opted to silence BoBe's proactive surfacing (floating bubble +
    /// TTS) without quitting the app. Session-scoped so a fresh launch
    /// always defaults to "BoBe is allowed to be proactive again." Voice
    /// playback is interrupted on toggle-on; resumes on next BoBe message
    /// when toggled off.
    var proactiveSilenced = false

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

    /// True ONLY while a screenshot is being taken THIS MOMENT (driven
    /// by SSE `screenCapture` indicator). Different from `isCapturing`,
    /// which is the persistent "capture system is enabled in settings"
    /// flag. Use this for status surfaces that should reflect what
    /// BoBe is doing right now (e.g. the avatar bubble "Capturing"
    /// label); use `isCapturing` for menus that toggle the setting.
    var isCaptureInProgress: Bool {
        self.context.captureInProgress
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

    /// The most recent assistant message received through the live SSE stream.
    /// Canonical history reloads intentionally do not update this value.
    private(set) var latestLiveBobeMessageId: String?

    func recordLiveBobeMessage(_ messageId: String) {
        self.latestLiveBobeMessageId = messageId
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

    var conversationEnding: Bool {
        self.context.conversationEnding
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
    var conversationReloadGeneration: UInt64 = 0
    var conversationMutationRevision: UInt64 = 0
    var streamMutationRevision: UInt64 = 0
    private var appNapActivity: NSObjectProtocol?
    private var backendObserverTask: Task<Void, Never>?
    private var sleepWakeObservers: [NSObjectProtocol] = []
    @ObservationIgnored var reconnectStatusTask: Task<Void, Never>?
    @ObservationIgnored lazy var toolExecutionController = ToolExecutionController { [weak self] mutation in
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
                    await self?.processBundle(bundle)
                },
                onConnectionChange: { [weak self] connected in
                    await self?.handleConnectionChange(connected)
                }
            )
        }
    }

    func disconnect() {
        self.conversationReloadGeneration &+= 1
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

    /// Toggles proactive surfacing. Setting to `true` interrupts any in-flight
    /// TTS so BoBe goes quiet immediately; setting to `false` re-enables the
    /// floating bubble for the next BoBe message but does not retroactively
    /// re-speak the current one (that would feel surprising).
    func toggleProactiveSilence() {
        self.proactiveSilenced.toggle()
        if self.proactiveSilenced {
            VoicePipeline.shared.interrupt()
        }
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
            id: "voice-\(Int(Date.now.timeIntervalSince1970 * 1000))",
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

    func sendMessage(_ content: String, requestId: UUID = UUID()) async {
        self.cancelConversationClear()
        let userMessage = ChatMessage(
            id: "user-\(requestId.uuidString.lowercased())",
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
            let response = try await self.client.sendMessage(content, requestId: requestId)
            self.updateState { ctx in
                if response.replayed {
                    Self.removeMessage(userMessage.id, messages: &ctx.messages)
                } else {
                    Self.markMessageSent(userMessage.id, messages: &ctx.messages)
                }
                // Optimistic lock — closes SSE-vs-daemon indicator race.
                ctx.acceptingUserMessages = response.requestStatus == "completed"
            }
            if response.replayed {
                await self.reloadCanonicalConversation()
            }
        } catch {
            // Admission conflicts are transient; unsafe replay conflicts are terminal.
            let isBusy409 = Self.isBusy409(error)
            let unsafeReplayMessage = Self.unsafeReplayMessage(error)
            bobeStoreLogger.error("sendMessage failed: \(error.localizedDescription)")
            self.updateState { ctx in
                Self.removeMessage(userMessage.id, messages: &ctx.messages)
                ctx.failedSendRecoveries.append(
                    FailedSendRecovery(
                        id: userMessage.id,
                        content: content,
                        requestId: requestId,
                        canRetry: unsafeReplayMessage == nil,
                        failureMessage: unsafeReplayMessage
                    )
                )
                if !isBusy409, unsafeReplayMessage == nil {
                    ctx.errorMessage = error.localizedDescription
                    ctx.daemonError = false
                }
                ctx.acceptingUserMessages = !isBusy409
            }
        }
    }

    private static func isBusy409(_ error: any Error) -> Bool {
        if case let DaemonError.httpError(statusCode, message, code) = error {
            return statusCode == 409
                && code == "CONFLICT"
                && message.localizedCaseInsensitiveContains("BoBe")
        }
        return false
    }

    private static func unsafeReplayMessage(_ error: any Error) -> String? {
        if case let DaemonError.httpError(_, message, code) = error,
           code == "REQUEST_REPLAY_UNSAFE" {
            return message
        }
        return nil
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
        guard recovery.canRetry else { return }

        self.updateState { ctx in
            ctx.failedSendRecoveries.removeAll { $0.id == recoveryId }
        }

        await self.sendMessage(recovery.content, requestId: recovery.requestId)
    }

    func clearMessages() {
        self.cancelConversationClear()
        self.updateState {
            $0.messages = []
            $0.lastMessage = nil
            $0.currentMessage = ""
        }
        self.latestLiveBobeMessageId = nil
        self.streamingMessage = ""
        self.streamingMessageId = nil
    }

    static func markMessageSent(_ messageId: String, messages: inout [ChatMessage]) {
        self.updateMessage(messageId, messages: &messages) { message in
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

    static func trimMessages(_ messages: inout [ChatMessage], capacity: Int = messageWindowCapacity) {
        guard capacity >= 0, messages.count > capacity else { return }
        let overflow = messages.count - capacity
        let protectedIds = Set(messages.filter { $0.isPending || $0.isStreaming }.map(\.id))
        let removable = messages.indices.filter { !protectedIds.contains(messages[$0].id) }
        for index in removable.prefix(overflow).reversed() {
            messages.remove(at: index)
        }
    }

    func updateState(_ block: (inout BobeContext) -> Void) {
        var ctx = self.context
        block(&ctx)
        Self.trimMessages(&ctx.messages)
        if ctx.messages != self.context.messages {
            self.conversationMutationRevision &+= 1
        }
        ctx.stateType = deriveStateType(from: ctx)
        // Recompute the derived displayable-message flag once per mutation rather
        // than walking the array on every body eval that reads it (the
        // overlay's `chatViewportFloorHeight` is the hot path).
        ctx.hasDisplayableBobeMessage = ctx.messages.contains(where: {
            $0.sender == .bobe && $0.belongsInConversationTrace
        })
        self.context = ctx
    }
}
