import AppKit
import SwiftUI

struct OverlayView: View {
    @Environment(\.accessibilityReduceMotion) var reduceMotion
    @State var store: BobeStore
    @State var themeStore: ThemeStore
    @State var chatPresentation: ChatPresentation = .collapsed
    @State var draftMessage = ""
    @State var lastMessageActivity: Date = .now
    @State var measuredContentSize: CGSize = .zero
    @State var inactivityTimer: Task<Void, Never>?
    @State var resizeTask: Task<Void, Never>?
    @State var composerFeedback: String?

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

    var isChatVisible: Bool {
        self.chatPresentation.isExpanded
    }

    var hasUnreadMessages: Bool {
        !self.store.messages.isEmpty && !self.isChatVisible
    }

    var overlaySectionTransition: AnyTransition {
        if OverlayMotionRuntime.reduceMotion {
            return .opacity
        }
        return .asymmetric(
            insertion: .opacity.combined(with: .scale(scale: 0.985, anchor: .bottomTrailing)),
            removal: .opacity
        )
    }

    var canAvatarToggleChat: Bool {
        self.store.stateType == .wantsToSpeak
    }

    var avatarClickAction: (() -> Void)? {
        guard self.canAvatarToggleChat else { return nil }
        return { self.handleAvatarClick() }
    }

    var avatarStateType: BobeStateType {
        if self.store.isInitialConnectionPending {
            return .loading
        }
        if !self.store.isConnected && !self.store.isBackendFatal {
            return .idle
        }
        return self.store.stateType
    }

    var statusTextOverride: String? {
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
        // Daemon-supplied progress label wins over generic "Thinking..." when present.
        if let label = self.store.indicatorMessage,
           !label.isEmpty,
           self.store.stateType == .thinking || self.store.stateType == .speaking {
            return label
        }
        return nil
    }

    var maxAllowedWindowHeight: CGFloat {
        guard let screen = OverlayWindowManager.shared.panel?.screen ?? NSScreen.main else {
            return WindowSizes.heightMax
        }
        return max(
            WindowSizes.heightCollapsed,
            screen.visibleFrame.height - (WindowSizes.margin * 2)
        )
    }

    var chatViewportMaxHeight: CGFloat {
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

    var chatViewportFloorHeight: CGFloat {
        self.store.messages.contains(where: { $0.sender == .bobe }) ? WindowSizes.heightChatViewportMin : 0
    }
}

enum ChatPresentation: Equatable {
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

enum ChatPresentationSource: Equatable {
    case automatic
    case manual
}

enum OverlayContentSizePreferenceKey: PreferenceKey {
    static let defaultValue: CGSize = .zero

    static func reduce(value: inout CGSize, nextValue: () -> CGSize) {
        let next = nextValue()
        value = CGSize(width: max(value.width, next.width), height: max(value.height, next.height))
    }
}
