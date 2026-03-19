import AppKit
import SwiftUI

struct OverlayView: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var store: BobeStore
    @State private var themeStore: ThemeStore
    @State private var chatPresentation: ChatPresentation = .collapsed
    @State private var draftMessage = ""
    @State private var lastMessageActivity: Date = .now
    @State private var measuredContentSize: CGSize = .zero
    @State private var inactivityTimer: Task<Void, Never>?
    @State private var resizeTask: Task<Void, Never>?
    @State private var composerFeedback: String?

    init(store: BobeStore, themeStore: ThemeStore = .shared) {
        self._store = State(initialValue: store)
        self._themeStore = State(initialValue: themeStore)
    }

    var body: some View {
        VStack(spacing: 0) {
            Spacer()

            self.overlayContent
            .background(
                GeometryReader { geo in
                    Color.clear
                        .preference(
                            key: OverlayContentSizePreferenceKey.self,
                            value: CGSize(width: ceil(geo.size.width), height: ceil(geo.size.height))
                        )
                }
            )
            .onPreferenceChange(OverlayContentSizePreferenceKey.self) { newSize in
                let sizeChanged =
                    abs(newSize.width - self.measuredContentSize.width) > 0.5
                    || abs(newSize.height - self.measuredContentSize.height) > 0.5
                if newSize.width > 0, newSize.height > 0, sizeChanged {
                    self.measuredContentSize = newSize
                    self.scheduleResizeWindow()
                }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomTrailing)
        .environment(\.theme, self.themeStore.currentTheme)
        .onChange(of: self.store.messages.count) { oldCount, newCount in
            self.handleMessagesChange(oldCount: oldCount, newCount: newCount)
            self.scheduleResizeWindow()
        }
        .onChange(of: self.isChatVisible) { _, _ in
            self.scheduleResizeWindow()
        }
        .onChange(of: self.store.toolExecutions.count) { _, _ in
            self.scheduleResizeWindow()
        }
        .onChange(of: self.store.composerBlockReason) { _, newReason in
            if let newReason, self.composerFeedback != nil {
                self.composerFeedback = self.waitingMessage(for: newReason)
            } else if newReason == nil {
                self.composerFeedback = nil
            }
        }
        .onChange(of: self.draftMessage) { _, newValue in
            if newValue.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                self.composerFeedback = nil
            }
        }
        .onChange(of: self.reduceMotion, initial: true) { _, new in
            OverlayMotionRuntime.reduceMotion = new
        }
        .onAppear {
            self.scheduleResizeWindow()
            self.startInactivityTimer()
        }
        .onDisappear {
            self.inactivityTimer?.cancel()
            self.resizeTask?.cancel()
        }
    }

    // MARK: - Derived State

    private var isChatVisible: Bool {
        self.chatPresentation.isExpanded
    }

    private var overlayContent: some View {
        VStack(spacing: 0) {
            self.chatHistorySection
            self.recoverySection
            self.composerSection
            self.errorBannerSection
            self.avatarSection
        }
    }

    @ViewBuilder
    private var chatHistorySection: some View {
        if self.isChatVisible, !self.store.messages.isEmpty {
            ChatStack(
                messages: self.store.messages,
                maxViewportHeight: self.chatViewportMaxHeight
            )
            .padding(.horizontal, 12)
            .transition(self.overlaySectionTransition)
        }
    }

    @ViewBuilder
    private var recoverySection: some View {
        if self.isChatVisible, !self.store.failedSendRecoveries.isEmpty {
            VStack(spacing: 8) {
                ForEach(self.store.failedSendRecoveries) { recovery in
                    FailedSendRecoveryBanner(
                        recovery: recovery,
                        onRetry: {
                            Task { await self.store.retryFailedSendRecovery(recovery.id) }
                        },
                        onDismiss: {
                            self.store.dismissFailedSendRecovery(recovery.id)
                        }
                    )
                }
            }
            .padding(.horizontal, 12)
            .padding(.bottom, 8)
            .transition(self.overlaySectionTransition)
        }
    }

    @ViewBuilder
    private var composerSection: some View {
        if self.isChatVisible {
            MessageInput(
                text: self.$draftMessage,
                onSend: self.handleSendMessage,
                onClose: { self.closeChat(userInitiated: true) },
                feedbackMessage: self.composerFeedback,
                isBusy: self.store.composerBlockReason != nil
            )
            .padding(.horizontal, 12)
            .zIndex(1)
            .transition(self.overlaySectionTransition)
        }
    }

    @ViewBuilder
    private var errorBannerSection: some View {
        if let error = self.store.errorMessage {
            HStack(spacing: 6) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .font(.system(size: 10))
                Text(error)
                    .bobeTextStyle(.overlayStatus)
                    .lineLimit(2)
                Spacer()
                Button {
                    self.store.dismissError()
                } label: {
                    Image(systemName: "xmark")
                        .font(.system(size: 8, weight: .bold))
                }
                .buttonStyle(.plain)
                .accessibilityLabel(L10n.tr("overlay.input.close.accessibility"))
            }
            .foregroundStyle(.white)
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(RoundedRectangle(cornerRadius: 8).fill(.red.opacity(0.85)))
            .padding(.horizontal, 12)
            .transition(self.overlaySectionTransition)
        }
    }

    @ViewBuilder
    private var avatarSection: some View {
        HStack(spacing: 12) {
            Spacer()

            ZStack(alignment: .topLeading) {
                ChatToggleButton(isActive: self.isChatVisible, action: self.toggleChatFromBubble)
                    .padding(.leading, 11)
                    .padding(.top, 23)
                    .zIndex(4)

                AvatarView(
                    stateType: self.avatarStateType,
                    isCapturing: self.store.isCapturing,
                    isConnected: self.store.isConnected,
                    hasMessage: self.hasUnreadMessages,
                    showInput: self.isChatVisible,
                    statusOverride: self.statusTextOverride,
                    isAvatarActionEnabled: self.canAvatarToggleChat,
                    onClick: self.avatarClickAction,
                    onToggleCapture: self.handleCaptureToggle
                )
                .padding(.top, 18)
                .padding(.leading, 16)
            }
            .frame(width: 148, height: 164, alignment: .topLeading)
        }
        .padding(.trailing, 12)
        .padding(.bottom, 8)
        .padding(.top, 0)
        .zIndex(3)
    }

    private var hasUnreadMessages: Bool {
        !self.store.messages.isEmpty && !self.isChatVisible
    }

    private var overlaySectionTransition: AnyTransition {
        if OverlayMotionRuntime.reduceMotion {
            return .opacity
        }
        return .asymmetric(
            insertion: .opacity.combined(with: .scale(scale: 0.985, anchor: .bottomTrailing)),
            removal: .opacity
        )
    }

    private var canAvatarToggleChat: Bool {
        self.store.stateType == .wantsToSpeak
    }

    private var avatarClickAction: (() -> Void)? {
        guard self.canAvatarToggleChat else { return nil }
        return { self.handleAvatarClick() }
    }

    private var avatarStateType: BobeStateType {
        if self.store.isInitialConnectionPending {
            return .loading
        }
        if !self.store.isConnected && !self.store.isBackendFatal {
            return .idle
        }
        return self.store.stateType
    }

    private var statusTextOverride: String? {
        if self.store.isReconnecting {
            return L10n.tr("overlay.reconnecting")
        }
        if self.store.isInitialConnectionPending {
            return L10n.tr("overlay.status.starting")
        }
        if self.store.capturePermissionMissing, self.store.stateType == .idle {
            return L10n.tr("overlay.status.capture_permission_needed")
        }
        if let tool = self.store.runningTools.first {
            return L10n.tr("overlay.status.using_tool_format", tool.toolName)
        }
        return nil
    }

    private var maxAllowedWindowHeight: CGFloat {
        guard let screen = OverlayWindowManager.shared.panel?.screen ?? NSScreen.main else {
            return WindowSizes.heightMax
        }
        return max(
            WindowSizes.heightCollapsed,
            screen.visibleFrame.height - (WindowSizes.margin * 2)
        )
    }

    private var chatViewportMaxHeight: CGFloat {
        let reserved =
            WindowSizes.heightAvatar
                + WindowSizes.heightInput
                + WindowSizes.heightExpandedChrome
        let available = self.maxAllowedWindowHeight - reserved
        return max(
            WindowSizes.heightChatViewportMin,
            min(WindowSizes.heightChatViewportMax, available)
        )
    }

    private var chatViewportFloorHeight: CGFloat {
        self.store.messages.contains(where: { $0.sender == .bobe }) ? WindowSizes.heightChatViewportMin : 0
    }

    // MARK: - Actions

    private func handleAvatarClick() {
        guard self.canAvatarToggleChat else { return }
        self.toggleChatManually()
    }

    private func handleCaptureToggle() {
        Task {
            _ = await self.store.toggleCapture()
        }
    }

    @discardableResult
    private func handleSendMessage(_ content: String) -> Bool {
        self.lastMessageActivity = .now

        if let blockReason = self.store.composerBlockReason {
            self.composerFeedback = self.waitingMessage(for: blockReason)
            return false
        }

        self.composerFeedback = nil
        Task {
            await self.store.sendMessage(content)
        }
        return true
    }

    private func handleMessagesChange(oldCount: Int, newCount: Int) {
        if newCount > oldCount, !self.isChatVisible, self.chatPresentation.allowsAutoOpen {
            if let last = self.store.messages.last, last.sender == .bobe {
                self.openChatAutomatically()
            }
        }
        if newCount == 0 {
            self.chatPresentation = .collapsed
            self.composerFeedback = nil
        }
        self.lastMessageActivity = .now
    }

    private func toggleChatFromBubble() {
        self.toggleChatManually()
    }

    private func toggleChatManually() {
        if self.isChatVisible {
            self.closeChat(userInitiated: true)
        } else {
            self.openChatManually()
        }
    }

    private func openChatManually() {
        self.setChatPresentation(.expanded(.manual))
    }

    private func openChatAutomatically() {
        guard self.chatPresentation.allowsAutoOpen else { return }
        self.setChatPresentation(.expanded(.automatic))
    }

    private func closeChat(userInitiated: Bool) {
        self.setChatPresentation(userInitiated ? .collapsedDismissed : .collapsed)
    }

    private func setChatPresentation(_ presentation: ChatPresentation) {
        guard self.chatPresentation != presentation else { return }
        self.chatPresentation = presentation
    }

    // MARK: - Window Sizing

    private func scheduleResizeWindow() {
        self.resizeTask?.cancel()
        self.resizeTask = Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(40))
            guard !Task.isCancelled else { return }
            self.resizeWindow()
        }
    }

    private func resizeWindow() {
        let size = self.calculateWindowSize()
        OverlayWindowManager.shared.resize(width: size.width, height: size.height)
    }

    private func calculateWindowSize() -> CGSize {
        let maxAllowedHeight = min(WindowSizes.heightMax, self.maxAllowedWindowHeight)
        let measuredWidth = max(WindowSizes.widthCollapsed, self.measuredContentSize.width)
        let measuredHeight = max(WindowSizes.heightCollapsed, self.measuredContentSize.height)

        if !self.isChatVisible {
            return CGSize(
                width: WindowSizes.widthCollapsed,
                height: min(measuredHeight, maxAllowedHeight)
            )
        }

        let minExpandedHeight =
            WindowSizes.heightAvatar
                + WindowSizes.heightInput
                + WindowSizes.heightExpandedChrome
                + self.chatViewportFloorHeight
        let measured = max(measuredContentSize.height, minExpandedHeight)
        let clampedHeight = min(measured, maxAllowedHeight)
        return CGSize(width: max(WindowSizes.widthExpanded, measuredWidth), height: clampedHeight)
    }

    // MARK: - Inactivity Timer

    private func startInactivityTimer() {
        self.inactivityTimer?.cancel()
        self.inactivityTimer = Task {
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(InactivityTiming.checkIntervalSeconds))
                guard !Task.isCancelled else { return }
                let elapsed = Date().timeIntervalSince(self.lastMessageActivity)
                if elapsed > InactivityTiming.timeoutSeconds, self.isChatVisible, self.store.stateType == .idle {
                    await MainActor.run {
                        self.closeChat(userInitiated: false)
                    }
                }
            }
        }
    }

    private func waitingMessage(for reason: MessageComposerBlockReason) -> String {
        switch reason {
        case .starting:
            return L10n.tr("overlay.input.waiting_for_starting")
        case .reconnecting:
            return L10n.tr("overlay.input.waiting_for_reconnecting")
        case .thinking:
            return L10n.tr("overlay.input.waiting_for_thinking")
        case .speaking:
            return L10n.tr("overlay.input.waiting_for_speaking")
        case .capturing:
            return L10n.tr("overlay.input.waiting_for_capturing")
        case .usingTool(let toolName):
            return L10n.tr("overlay.input.waiting_for_tool_format", toolName)
        }
    }
}

private enum ChatPresentation: Equatable {
    case collapsed
    case collapsedDismissed
    case expanded(ChatPresentationSource)

    var isExpanded: Bool {
        if case .expanded = self {
            return true
        }
        return false
    }

    var allowsAutoOpen: Bool {
        self != .collapsedDismissed
    }
}

private enum ChatPresentationSource: Equatable {
    case automatic
    case manual
}

private struct FailedSendRecoveryBanner: View {
    let recovery: FailedSendRecovery
    let onRetry: () -> Void
    let onDismiss: () -> Void

    @Environment(\.theme) private var theme

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: "exclamationmark.arrow.trianglehead.counterclockwise")
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(self.theme.colors.primary)
                .padding(.top, 2)

            Text(self.recovery.content)
                .bobeTextStyle(.chatBody)
                .foregroundStyle(self.theme.colors.text)
                .lineLimit(2)
                .multilineTextAlignment(.leading)
                .frame(maxWidth: .infinity, alignment: .leading)

            Button(L10n.tr("app.common.retry"), action: self.onRetry)
                .bobeTextStyle(.rowMeta)
                .buttonStyle(.plain)
                .foregroundStyle(self.theme.colors.primary)

            Button(action: self.onDismiss) {
                Image(systemName: "xmark")
                    .font(.system(size: 9, weight: .bold))
                    .foregroundStyle(self.theme.colors.textMuted)
            }
            .buttonStyle(.plain)
            .accessibilityLabel(L10n.tr("overlay.input.close.accessibility"))
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .background(
            RoundedRectangle(cornerRadius: 12)
                .fill(self.theme.colors.surface)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(self.theme.colors.border, lineWidth: 1)
        )
    }
}

private enum OverlayContentSizePreferenceKey: PreferenceKey {
    static let defaultValue: CGSize = .zero

    static func reduce(value: inout CGSize, nextValue: () -> CGSize) {
        let next = nextValue()
        value = CGSize(width: max(value.width, next.width), height: max(value.height, next.height))
    }
}
