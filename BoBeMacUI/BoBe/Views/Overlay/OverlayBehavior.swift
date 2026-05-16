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
        self.setChatPresentation(.expanded(.manual))
    }

    func openChatAutomatically() {
        guard self.chatPresentation.allowsAutoOpen else { return }
        self.setChatPresentation(.expanded(.automatic))
    }

    func closeChat(userInitiated: Bool) {
        self.setChatPresentation(userInitiated ? .collapsedDismissed : .collapsed)
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
                let elapsed = Date().timeIntervalSince(self.lastMessageActivity)
                if elapsed > InactivityTiming.timeoutSeconds, self.isChatVisible, self.store.stateType == .idle {
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
