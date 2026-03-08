import AppKit
import SwiftUI

struct OverlayView: View {
    @State private var store: BobeStore
    @State private var themeStore: ThemeStore
    @State private var showChat = false
    @State private var draftMessage = ""
    @State private var lastMessageActivity: Date = .now
    @State private var measuredContentSize: CGSize = .zero
    @State private var inactivityTimer: Task<Void, Never>?

    init(store: BobeStore, themeStore: ThemeStore = .shared) {
        self._store = State(initialValue: store)
        self._themeStore = State(initialValue: themeStore)
    }

    var body: some View {
        VStack(spacing: 0) {
            Spacer()

            VStack(spacing: 0) {
                if self.showChat, !self.store.messages.isEmpty {
                    ChatStack(
                        messages: self.store.messages,
                        maxViewportHeight: self.chatViewportMaxHeight
                    )
                    .padding(.horizontal, 12)
                    .transition(
                        .move(edge: .bottom)
                            .combined(with: .opacity)
                            .combined(with: .scale(scale: 0.96, anchor: .bottomTrailing))
                    )
                }

                if self.showChat {
                    if !self.store.failedSendRecoveries.isEmpty {
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
                        .transition(.move(edge: .bottom).combined(with: .opacity))
                    }

                    MessageInput(
                        text: self.$draftMessage,
                        onSend: self.handleSendMessage,
                        onClose: {
                            withAnimation(OverlayMotionRuntime.animation(for: .chatTransition)) {
                                self.showChat = false
                            }
                        },
                        isThinking: self.store.isThinking
                    )
                    .padding(.horizontal, 12)
                    .zIndex(2)
                    .transition(
                        .move(edge: .bottom)
                            .combined(with: .opacity)
                            .combined(with: .scale(scale: 0.94, anchor: .bottomTrailing))
                    )
                }

                if let error = store.errorMessage {
                    HStack(spacing: 6) {
                        Image(systemName: "exclamationmark.triangle.fill")
                            .font(.system(size: 10))
                        Text(error)
                            .font(.system(size: 10))
                            .lineLimit(2)
                        Spacer()
                        Button {
                            self.store.dismissError()
                        } label: {
                            Image(systemName: "xmark")
                                .font(.system(size: 8, weight: .bold))
                        }
                        .buttonStyle(.plain)
                    }
                    .foregroundStyle(.white)
                    .padding(.horizontal, 10)
                    .padding(.vertical, 6)
                    .background(RoundedRectangle(cornerRadius: 8).fill(.red.opacity(0.85)))
                    .padding(.horizontal, 12)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
                }

                HStack(spacing: 12) {
                    Spacer()

                    VStack(spacing: 4) {
                        AvatarView(
                            stateType: self.store.stateType,
                            isCapturing: self.store.isCapturing,
                            isConnected: self.store.isConnected,
                            hasMessage: self.hasUnreadMessages,
                            showInput: self.showChat,
                            statusOverride: self.statusTextOverride,
                            onClick: self.handleAvatarClick,
                            onToggleCapture: { Task { _ = await self.store.toggleCapture() } },
                            onToggleInput: {
                                withAnimation(OverlayMotionRuntime.animation(for: .chatTransition)) {
                                    self.showChat.toggle()
                                }
                            }
                        )

                        if self.store.isReconnecting {
                            Text(L10n.tr("overlay.reconnecting"))
                                .font(.system(size: 10))
                                .foregroundStyle(self.themeStore.currentTheme.colors.textMuted)
                                .transition(.opacity)
                        }
                    }
                }
                .padding(.trailing, 12)
                .padding(.bottom, 8)
                .padding(.top, self.showChat ? 6 : 0)
                .zIndex(1)
            }
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
                if newSize.width > 0, newSize.height > 0,
                   abs(newSize.width - self.measuredContentSize.width) > 0.5 || abs(newSize.height - self.measuredContentSize.height) > 0.5 {
                    self.measuredContentSize = newSize
                    self.resizeWindow()
                }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomTrailing)
        .environment(\.theme, self.themeStore.currentTheme)
        .onChange(of: self.store.messages.count) { oldCount, newCount in
            self.handleMessagesChange(oldCount: oldCount, newCount: newCount)
            self.resizeWindow()
        }
        .onChange(of: self.showChat) { _, _ in
            self.resizeWindow()
        }
        .onChange(of: self.store.toolExecutions.count) { _, _ in
            self.resizeWindow()
        }
        .onAppear {
            self.resizeWindow()
            self.startInactivityTimer()
        }
        .onDisappear {
            self.inactivityTimer?.cancel()
        }
    }

    // MARK: - Derived State

    private var hasUnreadMessages: Bool {
        !self.store.messages.isEmpty && !self.showChat
    }

    private var statusTextOverride: String? {
        if self.store.stateType == .loading {
            return L10n.tr("overlay.status.starting")
        }
        if self.store.isReconnecting {
            return L10n.tr("overlay.reconnecting")
        }
        if self.store.capturePermissionMissing, self.store.stateType == .idle {
            return L10n.tr("overlay.status.capture_permission_needed")
        }
        if let tool = store.runningTools.first {
            return L10n.tr("overlay.status.using_tool_format", tool.toolName)
        }
        return nil
    }

    // MARK: - Actions

    private func handleAvatarClick() {
        withAnimation(OverlayMotionRuntime.animation(for: .chatTransition)) {
            self.showChat.toggle()
        }
    }

    private func handleSendMessage(_ content: String) {
        self.lastMessageActivity = .now
        Task {
            await self.store.sendMessage(content)
        }
    }

    private func handleMessagesChange(oldCount: Int, newCount: Int) {
        if newCount > oldCount, !self.showChat {
            if let last = store.messages.last, last.sender == .bobe {
                withAnimation(OverlayMotionRuntime.animation(for: .chatTransition)) { self.showChat = true }
            }
        }
        self.lastMessageActivity = .now
    }

    // MARK: - Window Sizing

    private func resizeWindow() {
        let size = self.calculateWindowSize()
        OverlayWindowManager.shared.resize(width: size.width, height: size.height)
    }

    private func calculateWindowSize() -> CGSize {
        let maxAllowedHeight = min(WindowSizes.heightMax, self.maxAllowedWindowHeight)
        let measuredWidth = max(WindowSizes.widthCollapsed, self.measuredContentSize.width)
        let measuredHeight = max(WindowSizes.heightCollapsed, self.measuredContentSize.height)

        if !self.showChat {
            return CGSize(
                width: WindowSizes.widthCollapsed,
                height: min(measuredHeight, maxAllowedHeight)
            )
        }

        let minExpandedHeight =
            WindowSizes.heightAvatar
                + WindowSizes.heightInput
                + WindowSizes.heightExpandedChrome
                + (self.store.messages.isEmpty ? 0 : WindowSizes.heightChatViewportMin)
        let measured = max(measuredContentSize.height, minExpandedHeight)
        let clampedHeight = min(measured, maxAllowedHeight)
        return CGSize(width: max(WindowSizes.widthExpanded, measuredWidth), height: clampedHeight)
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

    // MARK: - Inactivity Timer

    private func startInactivityTimer() {
        self.inactivityTimer?.cancel()
        self.inactivityTimer = Task {
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(inactivityCheckIntervalSeconds))
                guard !Task.isCancelled else { return }
                let elapsed = Date().timeIntervalSince(self.lastMessageActivity)
                if elapsed > inactivityTimeoutSeconds, self.showChat, self.store.stateType == .idle {
                    await MainActor.run {
                        withAnimation(OverlayMotionRuntime.animation(for: .chatTransition)) { self.showChat = false }
                    }
                }
            }
        }
    }
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
                .font(.system(size: 12))
                .foregroundStyle(self.theme.colors.text)
                .lineLimit(2)
                .multilineTextAlignment(.leading)
                .frame(maxWidth: .infinity, alignment: .leading)

            Button(L10n.tr("app.common.retry"), action: self.onRetry)
                .font(.system(size: 11, weight: .semibold))
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
