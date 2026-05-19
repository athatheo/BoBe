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

    /// Pending auto-dismiss of the floating bubble. Set when BoBe stops
    /// being audible — gives the user ~1.2s to finish reading the reply
    /// after the audio fades, then quietly clears the bubble so the
    /// avatar returns to its resting state. See `OverlayBehavior`.
    @State var floatingBubbleAutoDismissTask: Task<Void, Never>?

    /// True while the cursor is hovering anywhere in the avatar area
    /// (bubble slot, avatar cluster, mic). Drives whether the chat-toggle
    /// and mic satellites are visible — at rest they're hidden so the
    /// avatar reads clean; on hover they fade in. Forced true while a
    /// satellite is actively in use (mic listening, chat open) so the
    /// affordance is never accidentally hidden under the user's hand.
    @State var avatarAreaHovered = false

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
            // Resize the NSPanel synchronously so the new sections render
            // INTO the expanded frame rather than the old collapsed one.
            // The `calculateWindowSize` floors handle the case where
            // measuredContentSize hasn't caught up yet.
            self.resizeWindowImmediate()
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
            // BoBe just stopped talking → start the post-speech grace
            // period before clearing the floating bubble. Gives the user
            // time to read the last sentence and dismiss it manually.
            // BoBe just started talking → cancel any pending dismiss so
            // the bubble stays put for the whole utterance.
            if audible {
                self.cancelFloatingBubbleAutoDismiss()
            } else if self.floatingBubbleMessage != nil {
                self.scheduleFloatingBubbleAutoDismiss(after: 1.2)
            }
        }
        .onChange(of: self.floatingBubbleMessage?.id) { _, newId in
            // A fresh bubble arrived while a dismiss was scheduled —
            // cancel the timer so the new message gets its own full
            // window starting when BoBe stops talking again.
            if newId != nil { self.cancelFloatingBubbleAutoDismiss() }
        }
        .onAppear {
            self.scheduleResizeWindow()
            self.startInactivityTimer()
        }
        .onDisappear {
            self.inactivityTimer?.cancel()
            self.resizeTask?.cancel()
            self.cancelFloatingBubbleAutoDismiss()
        }
    }

    // MARK: - Derived State

    var isChatVisible: Bool {
        self.chatPresentation.isExpanded
    }

    var hasUnreadMessages: Bool {
        !self.store.messages.isEmpty && !self.isChatVisible
    }

    /// The message currently shown in the floating bubble, looked up live so
    /// streaming content updates flow through. Returns `nil` when the chat
    /// is open, the message is silenced, or the stored id no longer exists.
    var floatingBubbleMessage: ChatMessage? {
        guard !self.isChatVisible,
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
        // Daemon-supplied progress label wins over generic "Thinking..." when present.
        if let label = self.store.indicatorMessage,
           !label.isEmpty,
           self.store.stateType == .thinking || self.store.stateType == .speaking {
            return label
        }
        return nil
    }

    /// Single source of truth for what the AvatarStatusBubble above the
    /// avatar should show right now. Strict priority order — only the
    /// highest-priority active mode renders; everything else is hidden.
    /// Returns `.hidden` whenever the chat surface is open (the chat
    /// itself owns the messaging duty in that mode, so a parallel bubble
    /// is duplication that looks broken).
    ///
    /// Daemon-disconnect errors are NOT emitted here — `errorBannerSection`
    /// owns them (it has the actionable Restart button). Routing them
    /// through both surfaces caused double-rendering when chat was closed.
    var bubbleMode: AvatarStatusBubble.Mode {
        if self.isChatVisible { return .hidden }
        // Live STT supersedes BoBe status — the user is mid-utterance and
        // the highest-value feedback is showing the words being heard.
        let pipeline = VoicePipeline.shared
        if pipeline.showPartialCaption, !pipeline.partialTranscript.isEmpty {
            return .userTranscribing(text: pipeline.partialTranscript)
        }
        // BoBe-content modes — actual reply takes precedence over status,
        // because content > status (you'd rather see the answer than a
        // "Thinking..." spinner).
        if let msg = self.floatingBubbleMessage { return .bobeMessage(msg) }

        // Status modes — anything BoBe is actively doing that the user
        // should be aware of. Order is "most blocking first."
        if self.store.isReconnecting {
            return .status(kind: .reconnecting, body: nil)
        }
        if self.store.isInitialConnectionPending {
            return .status(kind: .starting, body: nil)
        }
        if let tool = self.store.runningTools.first {
            return .status(kind: .toolRunning(name: tool.toolName), body: self.store.indicatorMessage)
        }
        if self.store.isThinking {
            return .status(kind: .thinking, body: self.store.indicatorMessage)
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
        // Read the cached flag from BobeContext — derived once per state
        // mutation in `BobeStore.updateState`. Previous version walked
        // `messages` O(N) on every body re-eval, which was wasted work
        // during streaming where the overlay re-evaluates often.
        self.store.context.hasBobeMessage ? WindowSizes.heightChatViewportMin : 0
    }
}

enum ChatPresentation: Equatable {
    case collapsed
    case expanded(ChatPresentationSource)

    var isExpanded: Bool {
        if case .expanded = self {
            return true
        }
        return false
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
