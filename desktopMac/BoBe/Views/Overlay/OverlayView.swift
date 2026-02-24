import SwiftUI

/// Main overlay window content. Based on OverlayWindow orchestrator.
/// Layout: bottom-right anchor, flex column items-end justify-end.
/// Indicator bubble sits to the LEFT of the avatar, vertically centered.
struct OverlayView: View {
    @State private var store = BobeStore.shared
    @State private var themeStore = ThemeStore.shared
    @State private var showChat = false
    @State private var prevMessagesCount = 0
    @State private var lastMessageActivity: Date = .now
    @State private var inactivityTimer: Task<Void, Never>?

    var body: some View {
        VStack(spacing: 0) {
            Spacer()

            // Chat stack (above avatar area)
            if showChat && !store.messages.isEmpty {
                ChatStack(messages: store.messages)
                    .padding(.horizontal, 12)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
            }

            // Message input (between chat and avatar)
            if showChat {
                MessageInput(
                    onSend: handleSendMessage,
                    onClose: { withAnimation { showChat = false } },
                    isThinking: store.isThinking
                )
                .padding(.horizontal, 12)
                .transition(.move(edge: .bottom).combined(with: .opacity))
            }

            // Avatar with indicator row
            // CSS: .avatar-with-indicator is relative, indicator is absolutely positioned left
            HStack(spacing: 12) {
                Spacer()

                // Indicator bubble (left of avatar)
                IndicatorBubble(
                    indicator: displayIndicator,
                    toolExecutions: store.toolExecutions
                )

                // Avatar (right side)
                AvatarView(
                    stateType: store.stateType,
                    isCapturing: store.isCapturing,
                    isConnected: store.isConnected,
                    hasMessage: hasUnreadMessages,
                    showInput: showChat,
                    onClick: handleAvatarClick,
                    onToggleCapture: { Task { _ = await store.toggleCapture() } },
                    onToggleInput: { withAnimation { showChat.toggle() } }
                )
            }
            .padding(.trailing, 12)
            .padding(.bottom, 8)
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
        .onAppear {
            startInactivityTimer()
        }
    }

    // MARK: - Derived State

    private var hasUnreadMessages: Bool {
        !store.messages.isEmpty && !showChat
    }

    private var displayIndicator: IndicatorType? {
        guard let indicator = store.activeIndicator else { return nil }
        if showChat && (indicator == .thinking || indicator == .analyzing) {
            return nil
        }
        return indicator
    }

    // MARK: - Actions

    private func handleAvatarClick() {
        withAnimation(.spring(duration: 0.3, bounce: 0.2)) {
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
                withAnimation { showChat = true }
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
        if !showChat {
            return CGSize(width: WindowSizes.widthCollapsed, height: WindowSizes.heightCollapsed)
        }

        let messageCount = min(store.messages.count, 2)
        var height = WindowSizes.heightAvatar + WindowSizes.heightInput
        height += CGFloat(messageCount) * WindowSizes.heightMessage
        height = min(height, WindowSizes.heightMax)

        return CGSize(width: WindowSizes.widthExpanded, height: height)
    }

    // MARK: - Inactivity Timer

    private func startInactivityTimer() {
        inactivityTimer?.cancel()
        inactivityTimer = Task {
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(inactivityCheckIntervalSeconds))
                let elapsed = Date().timeIntervalSince(lastMessageActivity)
                if elapsed > inactivityTimeoutSeconds && showChat {
                    await MainActor.run {
                        withAnimation { showChat = false }
                    }
                }
            }
        }
    }
}
