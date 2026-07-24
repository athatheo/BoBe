import AppKit
import SwiftUI

enum OverlaySatelliteFocus: Hashable {
    case chat
    case microphone
}

struct OverlayView: View {
    @Environment(\.accessibilityReduceMotion) var reduceMotion
    @State var store: BobeStore
    @State var themeStore: ThemeStore
    @State var isChatVisible = false
    @State var isRecentConversationVisible = false
    @State var draftMessage = ""
    @State var lastMessageActivity: Date = .now
    @State var measuredContentSize: CGSize = .zero
    @State var inactivityTimer: Task<Void, Never>?
    @State var resizeTask: Task<Void, Never>?
    @State var surfaceTransitionInProgress = false
    @State var surfaceTransitionGeneration = 0
    @State var composerFeedback: String?

    /// True while the cursor hovers over the chat surface. Pauses the
    /// inactivity-close timer so reading a long message doesn't dismiss
    /// the chat under the user's eyes.
    @State var isPointerOverChat = false

    /// True while the one-shot voice coachmark is visible. Set on the first
    /// listening transition per app install (gated by `VoiceCoachmarkFlag`).
    @State var voiceCoachmarkVisible = false

    /// ID of the BoBe message currently surfaced in the floating bubble next
    /// to the avatar. Set when a new BoBe message arrives while the chat is
    /// collapsed and proactive surfacing is enabled; cleared whenever the
    /// full chat opens or the user silences BoBe. Lookups go through
    /// `store.messages` so streaming updates flow through naturally.
    @State var floatingBubbleMessageId: String?
    @State var floatingBubbleDismissalStyle: AmbientBubbleDismissalStyle = .fade
    @State var floatingBubbleRecedeProgress: Double = 0
    @State var floatingBubbleTransitionGeneration = 0
    @State var floatingBubbleTransitionInProgress = false
    @State var floatingBubbleInteractionActive = false

    /// Pending auto-dismiss of the floating bubble. Scheduled only after
    /// streaming and audible speech finish, using a content-aware reading
    /// interval before the avatar returns to its resting state.
    @State var floatingBubbleAutoDismissTask: Task<Void, Never>?

    /// True while the cursor is hovering anywhere in the avatar area
    /// (bubble slot, avatar cluster, mic). Drives whether the chat-toggle
    /// and mic satellites are visible — at rest they're hidden so the
    /// avatar reads clean; on hover they fade in. Forced true while a
    /// satellite is actively in use (mic listening, chat open) so the
    /// affordance is never accidentally hidden under the user's hand.
    @State var avatarAreaHovered = false
    @FocusState var focusedSatellite: OverlaySatelliteFocus?

    init(store: BobeStore, themeStore: ThemeStore = .shared) {
        self._store = State(initialValue: store)
        self._themeStore = State(initialValue: themeStore)
    }

    var body: some View {
        self.lifecycleObservedOverlay
    }

    private var baseOverlay: some View {
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
    }

    private var conversationObservedOverlay: some View {
        self.baseOverlay
        .task {
            if SetupWindowManager.shared.consumeOpenChatAfterOnboarding() {
                await Task.yield()
                self.openChatManually()
            }
        }
        .onChange(of: self.store.messages.count) { oldCount, newCount in
            self.handleMessagesChange(oldCount: oldCount, newCount: newCount)
            self.scheduleResizeWindow()
        }
        .onChange(of: self.store.messages.last?.content) { _, _ in
            self.scheduleResizeWindow()
        }
        .onChange(of: self.store.latestLiveBobeMessageId) { _, messageId in
            self.handleLiveBobeMessageChange(messageId)
        }
        .onChange(of: self.store.toolExecutions.count) { _, _ in
            self.scheduleResizeWindow()
        }
        .onChange(of: self.store.failedSendRecoveries.count) { _, recoveryCount in
            if recoveryCount > 0, !self.isChatVisible {
                self.isRecentConversationVisible = false
                self.setChatVisible(true)
            }
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
    }

    private var activityObservedOverlay: some View {
        self.conversationObservedOverlay
        .onChange(of: self.store.stateType) { _, newState in
            // First-ever transition into "capturing" (mic open) → show the
            // voice coachmark once, then mark it seen so it never returns
            // for this user. Skip if reduce-motion is on (the rings/mouth
            // wouldn't be animated anyway, so the caption would just be
            // noise).
            if newState == .capturing,
               !VoiceCoachmarkFlag.hasSeen,
               !self.reduceMotion {
                VoiceCoachmarkFlag.markSeen()
                withAnimation(.easeOut(duration: 0.3)) {
                    self.voiceCoachmarkVisible = true
                }
                Task { @MainActor in
                    try? await Task.sleep(for: .seconds(6))
                    withAnimation(.easeOut(duration: 0.3)) {
                        self.voiceCoachmarkVisible = false
                    }
                }
            }
        }
        .onChange(of: self.reduceMotion, initial: true) { _, new in
            OverlayMotionRuntime.reduceMotion = new
        }
        .onChange(of: self.isTtsAudible) { _, audible in
            if audible {
                self.cancelFloatingBubbleAutoDismiss()
            } else {
                self.scheduleFloatingBubbleAutoDismissIfReady()
            }
        }
        .onChange(of: VoicePipeline.shared.state) { _, state in
            if state == .thinking || state == .speaking {
                self.cancelFloatingBubbleAutoDismiss()
            } else {
                self.scheduleFloatingBubbleAutoDismissIfReady()
            }
        }
    }

    private var bubbleObservedOverlay: some View {
        self.activityObservedOverlay
        .onChange(of: self.floatingBubbleMessage?.id) { _, newId in
            if newId == nil {
                self.cancelFloatingBubbleAutoDismiss()
            } else {
                self.scheduleFloatingBubbleAutoDismissIfReady()
            }
        }
        .onChange(of: self.floatingBubbleMessage?.isStreaming) { _, streaming in
            if streaming == false {
                self.scheduleFloatingBubbleAutoDismissIfReady()
            }
        }
        .onChange(of: self.avatarAreaHovered) { _, hovered in
            self.handleFloatingBubbleInteractionChange(
                hovered || self.floatingBubbleInteractionActive
            )
        }
        .onChange(of: self.floatingBubbleInteractionActive) { _, active in
            self.handleFloatingBubbleInteractionChange(active || self.avatarAreaHovered)
        }
        .onChange(of: self.isRecentConversationVisible) { _, visible in
            if visible {
                self.cancelFloatingBubbleAutoDismiss()
            } else {
                self.scheduleFloatingBubbleAutoDismissIfReady()
            }
            self.scheduleResizeWindow()
        }
    }

    private var lifecycleObservedOverlay: some View {
        self.bubbleObservedOverlay
        .onAppear {
            self.scheduleResizeWindow()
            self.startInactivityTimer()
        }
        .onDisappear {
            self.inactivityTimer?.cancel()
            self.resizeTask?.cancel()
            self.surfaceTransitionInProgress = false
            self.floatingBubbleTransitionInProgress = false
            self.cancelFloatingBubbleAutoDismiss()
        }
    }

    // MARK: - Derived State

    var ambientConversationMessages: [ChatMessage] {
        self.store.messages.filter(\.belongsInConversationTrace)
    }

    var hasUnreadMessages: Bool {
        !self.ambientConversationMessages.isEmpty && !self.isChatVisible
    }

    /// The message currently shown in the floating bubble, looked up live so
    /// streaming content updates flow through. Returns `nil` while recent
    /// history is expanded, when silenced, or when the id no longer exists.
    var floatingBubbleMessage: ChatMessage? {
        guard !self.isRecentConversationVisible,
              !self.store.proactiveSilenced,
              let id = self.floatingBubbleMessageId
        else { return nil }
        return self.store.messages.first(where: { $0.id == id })
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

    var avatarStateType: BobeStateType {
        if self.store.isInitialConnectionPending {
            return .loading
        }
        if !self.store.isConnected, !self.store.isBackendFatal {
            return .idle
        }
        // Treat active TTS playback as "speaking" so the wave ring, status
        // label, and eyes all stay in sync with audible BoBe output —
        // BobeStore.stateType only flips to .speaking during text streaming,
        // which ends well before the synthesized audio finishes.
        if self.isTtsAudible {
            return .speaking
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
        return nil
    }

    /// Single source of truth for what the AvatarStatusBubble above the
    /// avatar should show right now. Strict priority order — only the
    /// highest-priority active mode renders; everything else is hidden.
    /// Returns `.hidden` while recent history is explicitly expanded so the
    /// latest reply is never duplicated in two places.
    ///
    /// Daemon-disconnect errors are NOT emitted here — `errorBannerSection`
    /// owns them (it has the actionable Restart button). Routing them
    /// through both surfaces caused double-rendering when chat was closed.
    var bubbleMode: AvatarStatusBubble.Mode {
        if self.isRecentConversationVisible {
            return .hidden
        }
        // Live STT supersedes BoBe status — the user is mid-utterance and
        // the highest-value feedback is showing the words being heard.
        let pipeline = VoicePipeline.shared
        if pipeline.showPartialCaption, !pipeline.partialTranscript.isEmpty {
            return .userTranscribing(text: pipeline.partialTranscript)
        }
        // BoBe-content modes — actual reply takes precedence over status,
        // because content > status (you'd rather see the answer than a
        // "Thinking..." spinner).
        if let msg = self.floatingBubbleMessage {
            return .bobeMessage(msg)
        }

        // Status modes — anything BoBe is actively doing that the user
        // should be aware of. Order is "most blocking first."
        if self.store.isReconnecting {
            return .status(kind: .reconnecting, body: nil)
        }
        if self.store.isInitialConnectionPending {
            return .status(kind: .starting, body: nil)
        }
        if let tool = self.store.runningTools.first {
            return .status(kind: .toolRunning(name: tool.toolName), body: nil)
        }
        if self.store.isThinking {
            return .status(kind: .thinking, body: nil)
        }
        if self.store.isCaptureInProgress {
            return .status(kind: .capturing, body: nil)
        }
        if self.store.proactiveSilenced {
            return .silenced
        }
        return .hidden
    }

    /// True while TTS audio is actually playing right now. Gates the
    /// silence button in the bubble — there's nothing meaningful to
    /// silence when BoBe isn't speaking.
    ///
    /// Reads `VoicePipeline.isTtsAudible` rather than `ttsOutputLevel >
    /// threshold`. The pipeline applies hysteresis (rise 0.04 / fall 0.01
    /// with a 350 ms hangover) so this flag flips a handful of times per
    /// turn instead of at the RMS sample rate. Without the latch the
    /// OverlayView body re-evaluated ~50 Hz during speech, dragging every
    /// `.onChange` along with it.
    var isTtsAudible: Bool {
        let voice = VoicePipeline.shared
        return voice.state == .speaking || voice.isTtsAudible
    }

    var isVoiceResponseInProgress: Bool {
        let state = VoicePipeline.shared.state
        return state == .thinking || state == .speaking || state == .cancelling
    }

    var requiresWideAmbientWindow: Bool {
        self.floatingBubbleMessage != nil
            || (VoicePipeline.shared.showPartialCaption
                && !VoicePipeline.shared.partialTranscript.isEmpty)
            || self.store.softWarning != nil
            || self.store.context.daemonError
            || self.store.errorMessage != nil
    }

    /// True when the mic satellite should be visible. Pure hover — the
    /// user explicitly asked for the mic to be a hover-only affordance,
    /// not a state mirror. Voice state is already reflected in the avatar
    /// itself (presence ring, eyes, mouth) so the mic doesn't need to
    /// double as a state indicator.
    var showMicSatellite: Bool {
        self.avatarAreaHovered
    }

    /// True when the chat-toggle satellite should be visible — visible on
    /// hover OR when chat is open (so the user always has a way to close
    /// it without hunting).
    var showChatToggleSatellite: Bool {
        self.avatarAreaHovered || self.isChatVisible
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
        guard self.isRecentConversationVisible else { return 0 }
        // Read the cached flag from BobeContext — derived once per state
        // mutation in `BobeStore.updateState`. Previous version walked
        // `messages` O(N) on every body re-eval, which was wasted work
        // during streaming where the overlay re-evaluates often.
        return self.store.context.hasDisplayableBobeMessage
            ? WindowSizes.heightChatViewportMin
            : 0
    }
}

enum OverlayContentSizePreferenceKey: PreferenceKey {
    static let defaultValue: CGSize = .zero

    static func reduce(value: inout CGSize, nextValue: () -> CGSize) {
        let next = nextValue()
        value = CGSize(width: max(value.width, next.width), height: max(value.height, next.height))
    }
}
