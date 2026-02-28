import AppKit
import SwiftUI

/// Main overlay window content. Based on OverlayWindow orchestrator.
/// Layout: bottom-right anchor, flex column items-end justify-end.
/// Indicator bubble sits to the LEFT of the avatar, vertically centered.
struct OverlayView: View {
    @State private var store = BobeStore.shared
    @State private var themeStore = ThemeStore.shared
    @State private var showChat = false
    @State private var lastMessageActivity: Date = .now
    @State private var measuredContentSize: CGSize = .zero
    @State private var inactivityTimer: Task<Void, Never>?
    @State private var loadingPulseScale: CGFloat = 0.96

    var body: some View {
        VStack(spacing: 0) {
            Spacer()

            VStack(spacing: 0) {
                // Chat stack (above avatar area)
                if showChat && !store.messages.isEmpty {
                    ChatStack(
                        messages: store.messages,
                        maxViewportHeight: chatViewportMaxHeight
                    )
                        .padding(.horizontal, 12)
                        .transition(
                            .move(edge: .bottom)
                                .combined(with: .opacity)
                                .combined(with: .scale(scale: 0.96, anchor: .bottomTrailing))
                        )
                }

                // Message input (between chat and avatar)
                if showChat {
                    MessageInput(
                        onSend: handleSendMessage,
                        onClose: {
                            withAnimation(OverlayMotionRuntime.animation(for: .chatTransition)) {
                                showChat = false
                            }
                        },
                        isThinking: store.isThinking
                    )
                    .padding(.horizontal, 12)
                    .zIndex(2)
                    .transition(
                        .move(edge: .bottom)
                            .combined(with: .opacity)
                            .combined(with: .scale(scale: 0.94, anchor: .bottomTrailing))
                    )
                }

                // Error banner (dismissible)
                if let error = store.errorMessage {
                    HStack(spacing: 6) {
                        Image(systemName: "exclamationmark.triangle.fill")
                            .font(.system(size: 10))
                        Text(error)
                            .font(.system(size: 10))
                            .lineLimit(2)
                        Spacer()
                        Button {
                            // Dismiss error
                            BobeStore.shared.dismissError()
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

                // Avatar (right-aligned, no indicator bubble)
                HStack(spacing: 12) {
                    Spacer()

                    VStack(spacing: 4) {
                        AvatarView(
                            stateType: store.stateType,
                            isCapturing: store.isCapturing,
                            isConnected: store.isConnected,
                            hasMessage: hasUnreadMessages,
                            showInput: showChat,
                            statusOverride: statusTextOverride,
                            onClick: handleAvatarClick,
                            onToggleCapture: { Task { _ = await store.toggleCapture() } },
                            onToggleInput: {
                                withAnimation(OverlayMotionRuntime.animation(for: .chatTransition)) {
                                    showChat.toggle()
                                }
                            }
                        )
                        .opacity(store.stateType == .loading ? 0.7 : 1.0)
                        .scaleEffect(store.stateType == .loading ? loadingPulseScale : 1.0)
                        .animation(.easeInOut(duration: 1.4).repeatForever(autoreverses: true), value: loadingPulseScale)

                        if store.isReconnecting {
                            Text("Reconnecting...")
                                .font(.system(size: 10))
                                .foregroundStyle(themeStore.currentTheme.colors.textMuted)
                                .transition(.opacity)
                        }
                    }
                }
                .padding(.trailing, 12)
                .padding(.bottom, 8)
                .padding(.top, showChat ? 6 : 0)
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
                   abs(newSize.width - measuredContentSize.width) > 0.5 ||
                    abs(newSize.height - measuredContentSize.height) > 0.5 {
                    measuredContentSize = newSize
                    resizeWindow()
                }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomTrailing)
        .environment(\.theme, themeStore.currentTheme)
        .onChange(of: store.messages.count) { oldCount, newCount in
            handleMessagesChange(oldCount: oldCount, newCount: newCount)
            resizeWindow()
        }
        .onChange(of: showChat) { _, _ in
            resizeWindow()
        }
        .onChange(of: store.toolExecutions.count) { _, _ in
            resizeWindow()
        }
        .onAppear {
            resizeWindow()
            startInactivityTimer()
            if store.stateType == .loading {
                loadingPulseScale = 1.04
            }
        }
        .onDisappear {
            inactivityTimer?.cancel()
        }
    }

    // MARK: - Derived State

    private var hasUnreadMessages: Bool {
        !store.messages.isEmpty && !showChat
    }

    /// Text override for StatusLabel based on state priority
    private var statusTextOverride: String? {
        if store.stateType == .loading {
            return "Starting..."
        }
        if store.isReconnecting {
            return "Reconnecting..."
        }
        if store.capturePermissionMissing && store.stateType == .idle {
            return "Capture needs permission"
        }
        if let tool = store.runningTools.first {
            return "Using \(tool.toolName)..."
        }
        return nil
    }

    // MARK: - Actions

    private func handleAvatarClick() {
        withAnimation(OverlayMotionRuntime.animation(for: .chatTransition)) {
            showChat.toggle()
        }
    }

    private func handleSendMessage(_ content: String) {
        lastMessageActivity = .now
        Task { _ = await store.sendMessage(content) }
    }

    private func handleMessagesChange(oldCount: Int, newCount: Int) {
        if newCount > oldCount && !showChat {
            if let last = store.messages.last, last.sender == .bobe {
                withAnimation(OverlayMotionRuntime.animation(for: .chatTransition)) { showChat = true }
            }
        }
        lastMessageActivity = .now
    }

    // MARK: - Window Sizing

    private func resizeWindow() {
        let size = calculateWindowSize()
        OverlayWindowManager.shared.resize(width: size.width, height: size.height)
    }

    private func calculateWindowSize() -> CGSize {
        let maxAllowedHeight = min(WindowSizes.heightMax, maxAllowedWindowHeight)
        let measuredWidth = max(WindowSizes.widthCollapsed, measuredContentSize.width)
        let measuredHeight = max(WindowSizes.heightCollapsed, measuredContentSize.height)

        if !showChat {
            return CGSize(
                width: WindowSizes.widthCollapsed,
                height: min(measuredHeight, maxAllowedHeight)
            )
        }

        let minExpandedHeight =
            WindowSizes.heightAvatar
            + WindowSizes.heightInput
            + WindowSizes.heightExpandedChrome
            + (store.messages.isEmpty ? 0 : WindowSizes.heightChatViewportMin)
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
        let available = maxAllowedWindowHeight - reserved
        return max(
            WindowSizes.heightChatViewportMin,
            min(WindowSizes.heightChatViewportMax, available)
        )
    }

    // MARK: - Inactivity Timer

    private func startInactivityTimer() {
        inactivityTimer?.cancel()
        inactivityTimer = Task {
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(inactivityCheckIntervalSeconds))
                guard !Task.isCancelled else { return }
                let elapsed = Date().timeIntervalSince(lastMessageActivity)
                if elapsed > inactivityTimeoutSeconds && showChat && store.stateType == .idle {
                    await MainActor.run {
                        withAnimation(OverlayMotionRuntime.animation(for: .chatTransition)) { showChat = false }
                    }
                }
            }
        }
    }
}

private enum OverlayContentSizePreferenceKey: PreferenceKey {
    static let defaultValue: CGSize = .zero

    static func reduce(value: inout CGSize, nextValue: () -> CGSize) {
        let next = nextValue()
        value = CGSize(width: max(value.width, next.width), height: max(value.height, next.height))
    }
}
