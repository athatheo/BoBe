import Foundation

extension OverlayView {
    func handleAvatarClick() {
        guard self.canAvatarToggleChat else { return }
        self.toggleChatManually()
    }

    func handleCaptureToggle() {
        Task {
            _ = await self.store.toggleCapture()
        }
    }

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
        return true
    }

    func handleMessagesChange(oldCount: Int, newCount: Int) {
        if newCount > oldCount, !self.isChatVisible {
            if let last = self.store.messages.last, last.sender == .bobe {
                // Replace the previous floating bubble target with the
                // freshest BoBe message. We never *auto-open* the full
                // chat anymore — the floating bubble carries the
                // surfacing job and the user clicks it (or the avatar)
                // to expand.
                self.surfaceFloatingBubble(for: last)
            }
        }
        if newCount == 0 {
            self.chatPresentation = .collapsed
            self.composerFeedback = nil
            self.dismissFloatingBubble()
        }
        self.lastMessageActivity = .now
    }

    func toggleChatFromBubble() {
        self.toggleChatManually()
    }

    func toggleChatManually() {
        if self.isChatVisible {
            self.closeChat(userInitiated: true)
        } else {
            self.openChatManually()
        }
    }

    func openChatManually() {
        // Pre-size the panel BEFORE flipping state so SwiftUI's next body
        // evaluation renders the chat sections into a window that already
        // has room. Relying on `.onChange(of: isChatVisible)` alone leaves
        // a one-frame window where the new sections are drawn clipped
        // inside the old collapsed frame (visible as a "small square
        // stuck for a beat" stutter). Doing it inline in the action
        // handler removes any dependency on SwiftUI's update ordering.
        self.preflightExpandWindow()
        self.setChatPresentation(.expanded(.manual))
        self.dismissFloatingBubble()
    }

    func openChatAutomatically() {
        self.preflightExpandWindow()
        self.setChatPresentation(.expanded(.automatic))
        self.dismissFloatingBubble()
    }

    /// Resize the NSPanel synchronously to its expanded floor BEFORE any
    /// state change so the chat sections never render into a too-small
    /// frame. Width uses `widthExpanded`; height uses the same floor
    /// `calculateWindowSize` would compute for an expanded state.
    private func preflightExpandWindow() {
        let floorHeight = WindowSizes.heightAvatar
            + WindowSizes.heightInput
            + WindowSizes.heightExpandedChrome
            + self.chatViewportFloorHeight
        let height = min(floorHeight, self.maxAllowedWindowHeight)
        OverlayWindowManager.shared.resize(width: WindowSizes.widthExpanded, height: height)
    }

    func closeChat(userInitiated _: Bool) {
        // Closing the chat hides the bubble UI only. Voice listening + TTS
        // playback live in `VoicePipeline` and are toggled explicitly by
        // the mic button + the global voice setting — they intentionally
        // outlive the chat surface so BoBe can keep speaking (and being
        // heard) with the chat collapsed, matching the floating-bubble
        // model. Previously we disconnected here, which silently killed
        // all audio whenever the user dismissed the chat panel.
        self.setChatPresentation(.collapsed)
        // If there's a BoBe message in flight (currently streaming or just
        // arrived), surface it as the floating bubble so the user keeps
        // seeing the reply even after collapsing the chat. Otherwise
        // clear any stale bubble target.
        if let last = self.store.messages.last, last.sender == .bobe {
            self.surfaceFloatingBubble(for: last)
        } else {
            self.dismissFloatingBubble()
        }
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
    private func surfaceFloatingBubble(for message: ChatMessage) {
        guard !self.store.proactiveSilenced else { return }
        self.floatingBubbleMessageId = message.id
        self.cancelFloatingBubbleAutoDismiss()
    }

    /// Single clear point. Cancels any pending auto-dismiss so a later
    /// timer tick doesn't no-op against an already-cleared bubble.
    func dismissFloatingBubble() {
        self.floatingBubbleMessageId = nil
        self.cancelFloatingBubbleAutoDismiss()
    }

    /// Schedule the bubble to clear `delay` seconds from now. Used by
    /// the post-speech watcher in `OverlayView` — keeps the reply on
    /// screen briefly after BoBe finishes talking, then fades it out so
    /// the avatar can return to its resting state. Coalesces: a second
    /// call within the delay window resets the timer rather than
    /// stacking.
    func scheduleFloatingBubbleAutoDismiss(after delay: TimeInterval) {
        self.cancelFloatingBubbleAutoDismiss()
        self.floatingBubbleAutoDismissTask = Task { @MainActor in
            try? await Task.sleep(for: .seconds(delay))
            guard !Task.isCancelled else { return }
            self.floatingBubbleMessageId = nil
            self.floatingBubbleAutoDismissTask = nil
        }
    }

    /// Cancel a pending auto-dismiss. Safe to call when none is queued.
    func cancelFloatingBubbleAutoDismiss() {
        self.floatingBubbleAutoDismissTask?.cancel()
        self.floatingBubbleAutoDismissTask = nil
    }

    func setChatPresentation(_ presentation: ChatPresentation) {
        guard self.chatPresentation != presentation else { return }
        self.chatPresentation = presentation
    }

    func scheduleResizeWindow() {
        self.resizeTask?.cancel()
        self.resizeTask = Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(40))
            guard !Task.isCancelled else { return }
            self.resizeWindow()
        }
    }

    /// Bypass the 40ms debounce. Called on `isChatVisible` transitions so the
    /// NSPanel reaches its expanded floor BEFORE SwiftUI starts inserting the
    /// chat / composer sections — otherwise the new content is briefly drawn
    /// inside the old collapsed frame and looks "stuck in a small square"
    /// until the next debounced resize catches up.
    func resizeWindowImmediate() {
        self.resizeTask?.cancel()
        self.resizeTask = nil
        self.resizeWindow()
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
