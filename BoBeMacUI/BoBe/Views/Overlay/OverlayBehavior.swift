import Foundation
import SwiftUI

extension OverlayView {
    @discardableResult
    func handleSendMessage(_ content: String) -> Bool {
        self.lastMessageActivity = .now

        if let blockReason = self.store.composerBlockReason {
            self.composerFeedback = self.waitingMessage(for: blockReason)
            return false
        }

        self.composerFeedback = nil
        Task {
            await self.store.sendMessage(content)
        }
        self.isRecentConversationVisible = false
        self.setChatVisible(false)
        self.dismissFloatingBubble()
        return true
    }

    func handleMessagesChange(oldCount _: Int, newCount: Int) {
        if newCount == 0 {
            self.setChatVisible(false)
            self.composerFeedback = nil
            self.dismissFloatingBubble()
        }
        self.lastMessageActivity = .now
    }

    func handleLiveBobeMessageChange(_ messageId: String?) {
        guard let messageId,
              let message = self.store.messages.first(where: { $0.id == messageId })
        else { return }
        self.surfaceFloatingBubble(for: message)
        self.scheduleResizeWindow()
    }

    func toggleChatManually() {
        if self.isChatVisible {
            self.closeChat(userInitiated: true)
        } else {
            self.openChatManually()
        }
    }

    func openChatManually() {
        self.isRecentConversationVisible = false
        self.setChatVisible(true)
    }

    func openCurrentAnswer() {
        self.cancelFloatingBubbleAutoDismiss()
        self.isRecentConversationVisible = true
        if self.isChatVisible {
            self.preflightExpandWindow()
            self.dismissFloatingBubble()
            self.scheduleResizeWindow()
        } else {
            self.setChatVisible(true)
            self.dismissFloatingBubble()
        }
    }

    /// Resize the NSPanel synchronously to its expanded floor BEFORE any
    /// state change so the chat sections never render into a too-small
    /// frame. Width uses `widthExpanded`; height uses the same floor
    /// `calculateWindowSize` would compute for an expanded state.
    private func preflightExpandWindow() {
        let floorHeight = WindowSizes.heightAvatar
            + WindowSizes.heightInput
            + (self.isRecentConversationVisible ? WindowSizes.heightExpandedChrome : 8)
            + self.chatViewportFloorHeight
        let height = min(floorHeight, self.maxAllowedWindowHeight)
        let width = self.isRecentConversationVisible
            ? WindowSizes.widthExpanded
            : WindowSizes.widthComposer
        OverlayWindowManager.shared.resize(width: width, height: height)
    }

    private func preflightAmbientAnswerWindow() {
        OverlayWindowManager.shared.resize(
            width: WindowSizes.widthComposer,
            height: min(WindowSizes.heightAmbientAnswer, self.maxAllowedWindowHeight)
        )
    }

    func closeChat(userInitiated: Bool) {
        // Closing the chat hides the bubble UI only. Voice listening + TTS
        // playback live in `VoicePipeline` and are toggled explicitly by
        // the mic button + the global voice setting — they intentionally
        // outlive the chat surface so BoBe can keep speaking (and being
        // heard) with the chat collapsed, matching the floating-bubble
        // model. Previously we disconnected here, which silently killed
        // all audio whenever the user dismissed the chat panel.
        self.setChatVisible(false)
        self.isRecentConversationVisible = false
        if userInitiated {
            self.dismissFloatingBubble()
            return
        }
        // Automatic collapse preserves an already-visible live reply but
        // never resurrects content from canonical history.
    }

    /// Toggle proactive surfacing. When silenced we drop the current
    /// floating bubble; when un-silenced the next BoBe message will
    /// repopulate it.
    func toggleProactiveSilence() {
        self.store.toggleProactiveSilence()
        if self.store.proactiveSilenced {
            self.dismissFloatingBubble()
        }
    }

    // MARK: - Floating bubble lifecycle

    /// Single setter for the floating bubble target. Refuses to surface
    /// while proactive silencing is on — that mode means "don't pop up
    /// at me, I'll open the chat when I want." Centralizing the
    /// silenced check here keeps callers from duplicating it (and from
    /// forgetting it, which previously caused bubbles to flash through
    /// silenced mode for one frame).
    func surfaceFloatingBubble(for message: ChatMessage) {
        guard message.hasVisibleContent,
              !self.store.proactiveSilenced,
              !self.isRecentConversationVisible
        else { return }
        self.preflightAmbientAnswerWindow()
        self.floatingBubbleDismissalStyle = .fade
        self.floatingBubbleRecedeProgress = 0
        self.floatingBubbleTransitionGeneration &+= 1
        withAnimation(OverlayMotionRuntime.animation(for: .chatTransition)) {
            self.floatingBubbleMessageId = message.id
        }
        self.cancelFloatingBubbleAutoDismiss()
        self.scheduleFloatingBubbleAutoDismissIfReady()
    }

    /// Single clear point. Cancels any pending auto-dismiss so a later
    /// timer tick doesn't no-op against an already-cleared bubble.
    func dismissFloatingBubble(style: AmbientBubbleDismissalStyle = .fade) {
        self.cancelFloatingBubbleAutoDismiss()
        self.floatingBubbleDismissalStyle = style
        guard self.floatingBubbleMessageId != nil else {
            return
        }

        self.floatingBubbleTransitionGeneration &+= 1
        let generation = self.floatingBubbleTransitionGeneration

        guard style == .stardust else {
            var transaction = Transaction()
            transaction.disablesAnimations = true
            withTransaction(transaction) {
                self.floatingBubbleMessageId = nil
                self.floatingBubbleRecedeProgress = 0
            }
            self.floatingBubbleTransitionInProgress = false
            self.scheduleResizeWindow()
            return
        }

        self.floatingBubbleTransitionInProgress = true
        let duration = OverlayMotionRuntime.reduceMotion
            ? AmbientBubbleTransition.fadeDurationSeconds
            : AmbientBubbleTransition.stardustDurationSeconds
        let animation = OverlayMotionRuntime.reduceMotion
            ? Animation.easeOut(duration: duration)
            : Animation.easeIn(duration: duration)

        withAnimation(animation, completionCriteria: .logicallyComplete) {
            self.floatingBubbleRecedeProgress = 1
        } completion: {
            guard generation == self.floatingBubbleTransitionGeneration else {
                return
            }
            var transaction = Transaction()
            transaction.disablesAnimations = true
            withTransaction(transaction) {
                self.floatingBubbleMessageId = nil
                self.floatingBubbleRecedeProgress = 0
            }
            self.floatingBubbleTransitionInProgress = false
            self.scheduleResizeWindow()
        }
    }

    func scheduleFloatingBubbleAutoDismissIfReady() {
        guard let message = self.floatingBubbleMessage,
              AmbientMessageTiming.shouldAutoDismiss(message),
              !message.isStreaming,
              !self.isTtsAudible,
              !self.isVoiceResponseInProgress,
              !self.avatarAreaHovered,
              !self.floatingBubbleInteractionActive,
              !self.isRecentConversationVisible
        else {
            return
        }
        self.scheduleFloatingBubbleAutoDismiss(
            after: AmbientMessageTiming.dwellDuration(for: message.content)
        )
    }

    func scheduleFloatingBubbleAutoDismiss(after delay: TimeInterval) {
        self.cancelFloatingBubbleAutoDismiss()
        self.floatingBubbleAutoDismissTask = Task { @MainActor in
            try? await Task.sleep(for: .seconds(delay))
            guard !Task.isCancelled else { return }
            self.floatingBubbleAutoDismissTask = nil
            self.dismissFloatingBubble(style: .stardust)
        }
    }

    /// Cancel a pending auto-dismiss. Safe to call when none is queued.
    func cancelFloatingBubbleAutoDismiss() {
        self.floatingBubbleAutoDismissTask?.cancel()
        self.floatingBubbleAutoDismissTask = nil
    }

    func handleFloatingBubbleInteractionChange(_ active: Bool) {
        if active {
            self.cancelFloatingBubbleAutoDismiss()
            guard self.floatingBubbleRecedeProgress > 0 else {
                return
            }
            self.floatingBubbleTransitionGeneration &+= 1
            self.floatingBubbleTransitionInProgress = false
            withAnimation(
                OverlayMotionRuntime.reduceMotion
                    ? .easeOut(duration: 0.1)
                    : .easeOut(duration: 0.14)
            ) {
                self.floatingBubbleRecedeProgress = 0
            }
        } else {
            self.scheduleFloatingBubbleAutoDismissIfReady()
        }
    }

    func scheduleResizeWindow() {
        guard !self.surfaceTransitionInProgress,
              !self.floatingBubbleTransitionInProgress
        else {
            return
        }
        self.resizeTask?.cancel()
        self.resizeTask = Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(40))
            guard !Task.isCancelled else { return }
            self.resizeWindow()
        }
    }

    func setChatVisible(_ visible: Bool) {
        guard self.isChatVisible != visible else { return }
        self.resizeTask?.cancel()
        self.resizeTask = nil
        if visible {
            self.preflightExpandWindow()
        }

        self.surfaceTransitionGeneration &+= 1
        let generation = self.surfaceTransitionGeneration
        self.surfaceTransitionInProgress = true
        let updates = {
            if !visible {
                self.isRecentConversationVisible = false
            }
            self.isChatVisible = visible
        }
        guard let animation = OverlayMotionRuntime.animation(for: .chatTransition) else {
            updates()
            self.surfaceTransitionInProgress = false
            self.resizeWindow()
            return
        }
        withAnimation(animation, completionCriteria: .logicallyComplete) {
            updates()
        } completion: {
            guard generation == self.surfaceTransitionGeneration else {
                return
            }
            self.surfaceTransitionInProgress = false
            self.resizeWindow()
        }
    }

    func resizeWindow() {
        let size = self.calculateWindowSize()
        OverlayWindowManager.shared.resize(width: size.width, height: size.height)
    }

    func calculateWindowSize() -> CGSize {
        let maxAllowedHeight = min(WindowSizes.heightMax, self.maxAllowedWindowHeight)
        let measuredWidth = max(WindowSizes.widthCollapsed, self.measuredContentSize.width)
        let measuredHeight = max(WindowSizes.heightCollapsed, self.measuredContentSize.height)

        if !self.isChatVisible {
            if self.floatingBubbleMessage != nil {
                return CGSize(
                    width: WindowSizes.widthComposer,
                    height: min(WindowSizes.heightAmbientAnswer, maxAllowedHeight)
                )
            }
            return CGSize(
                width: self.requiresWideAmbientWindow
                    ? WindowSizes.widthComposer
                    : WindowSizes.widthCollapsed,
                height: min(measuredHeight, maxAllowedHeight)
            )
        }

        let ambientChromeHeight: CGFloat = self.isRecentConversationVisible
            ? WindowSizes.heightExpandedChrome
            : 8
        let minExpandedHeight =
            WindowSizes.heightAvatar
                + WindowSizes.heightInput
                + ambientChromeHeight
                + self.chatViewportFloorHeight
        let measured = max(measuredContentSize.height, minExpandedHeight)
        let clampedHeight = min(measured, maxAllowedHeight)
        let targetWidth = self.isRecentConversationVisible
            ? WindowSizes.widthExpanded
            : WindowSizes.widthComposer
        return CGSize(width: max(targetWidth, measuredWidth), height: clampedHeight)
    }

    func toggleRecentConversation() {
        if !self.isRecentConversationVisible {
            OverlayWindowManager.shared.resize(
                width: WindowSizes.widthExpanded,
                height: min(self.maxAllowedWindowHeight, self.calculateWindowSize().height)
            )
        }
        withAnimation(OverlayMotionRuntime.animation(for: .chatTransition)) {
            self.isRecentConversationVisible.toggle()
        }
        self.lastMessageActivity = .now
    }

    func startInactivityTimer() {
        self.inactivityTimer?.cancel()
        self.inactivityTimer = Task {
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(InactivityTiming.checkIntervalSeconds))
                guard !Task.isCancelled else { return }
                let elapsed = Date.now.timeIntervalSince(self.lastMessageActivity)
                if elapsed > InactivityTiming.timeoutSeconds,
                   self.isChatVisible,
                   self.store.stateType == .idle,
                   !self.isPointerOverChat {
                    await MainActor.run {
                        self.closeChat(userInitiated: false)
                    }
                }
            }
        }
    }

    func waitingMessage(for reason: MessageComposerBlockReason) -> String {
        switch reason {
        case .starting:
            L10n.tr("overlay.input.waiting_for_starting")
        case .reconnecting:
            L10n.tr("overlay.input.waiting_for_reconnecting")
        case .thinking:
            L10n.tr("overlay.input.waiting_for_thinking")
        case .speaking:
            L10n.tr("overlay.input.waiting_for_speaking")
        case .capturing:
            L10n.tr("overlay.input.waiting_for_capturing")
        case let .usingTool(toolName):
            L10n.tr("overlay.input.waiting_for_tool_format", toolName)
        }
    }
}

enum AmbientMessageTiming {
    static let minimumDwellSeconds: TimeInterval = 8
    static let maximumAutoDismissWords = 60

    static func dwellDuration(for content: String) -> TimeInterval {
        let wordCount = content.split(whereSeparator: \.isWhitespace).count
        return max(self.minimumDwellSeconds, 2 + Double(wordCount) / 3.3)
    }

    static func shouldAutoDismiss(_ message: ChatMessage) -> Bool {
        guard message.responseOrigin == .proactive, message.isComplete else {
            return false
        }
        return message.content.split(whereSeparator: \.isWhitespace).count
            <= self.maximumAutoDismissWords
    }
}
