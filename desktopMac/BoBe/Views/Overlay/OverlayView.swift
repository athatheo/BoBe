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

    var body: some View {
        VStack(spacing: 0) {
            Spacer()

            VStack(spacing: 0) {
                // Chat stack (above avatar area)
                if showChat && !store.messages.isEmpty {
                    ChatStack(messages: store.messages)
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

                    AvatarView(
                        stateType: store.stateType,
                        isCapturing: store.isCapturing,
                        isConnected: store.isConnected,
                        hasMessage: hasUnreadMessages,
                        showInput: showChat,
                        statusOverride: toolStatusText,
                        onClick: handleAvatarClick,
                        onToggleCapture: { Task { _ = await store.toggleCapture() } },
                        onToggleInput: {
                            withAnimation(OverlayMotionRuntime.animation(for: .chatTransition)) {
                                showChat.toggle()
                            }
                        }
                    )
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
        }
        .onDisappear {
            inactivityTimer?.cancel()
        }
    }

    // MARK: - Derived State

    private var hasUnreadMessages: Bool {
        !store.messages.isEmpty && !showChat
    }

    /// Text override for StatusLabel when tools are running (e.g. "Using search_memories...")
    private var toolStatusText: String? {
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
        let measuredWidth = max(WindowSizes.widthCollapsed, measuredContentSize.width)
        let measuredHeight = max(WindowSizes.heightCollapsed, measuredContentSize.height)

        if !showChat {
            return CGSize(width: WindowSizes.widthCollapsed, height: measuredHeight)
        }

        let minExpandedHeight = WindowSizes.heightAvatar + WindowSizes.heightInput
        let measured = max(measuredContentSize.height, minExpandedHeight)
        let clampedHeight = min(measured, WindowSizes.heightMax)
        return CGSize(width: max(WindowSizes.widthExpanded, measuredWidth), height: clampedHeight)
    }

    // MARK: - Inactivity Timer

    private func startInactivityTimer() {
        inactivityTimer?.cancel()
        inactivityTimer = Task {
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(inactivityCheckIntervalSeconds))
                let elapsed = Date().timeIntervalSince(lastMessageActivity)
                if elapsed > inactivityTimeoutSeconds && showChat && store.stateType == .idle {
                    await MainActor.run {
                        withAnimation { showChat = false }
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
