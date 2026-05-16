import Foundation

extension BobeStore {
    func processBundle(_ bundle: StreamBundle) {
        switch bundle.type {
        case .indicator:
            if let payload = try? bundle.payload.decode(as: IndicatorPayload.self) {
                self.handleIndicator(payload)
            }
        case .textDelta:
            // `done: true` is the end-of-turn marker.
            if let payload = try? bundle.payload.decode(as: TextDeltaPayload.self) {
                self.handleTextDelta(payload, messageId: bundle.messageId)
            }
        case .toolCallStart, .toolCallComplete:
            self.handleToolCall(bundle.payload)
        case .conversationClosed:
            if let payload = try? bundle.payload.decode(as: ConversationClosedPayload.self) {
                self.handleConversationClosed(payload)
            }
        case .error:
            if let payload = try? bundle.payload.decode(as: ErrorPayload.self) {
                self.handleErrorPayload(payload)
            }
        case .heartbeat, .unknown:
            break
        }
    }

    private func handleIndicator(_ payload: IndicatorPayload) {
        let indicator = payload.indicator

        if indicator == .idle, !self.context.currentMessage.isEmpty {
            self.finalizeStreamingMessage()
            return
        }

        let activeIndicator: IndicatorType? = (indicator == .thinking) ? .thinking : nil

        self.updateState { ctx in
            switch indicator {
            case .idle:
                ctx.captureInProgress = false
                ctx.thinking = false
                ctx.speaking = false
                ctx.acceptingUserMessages = true
            case .screenCapture:
                ctx.captureInProgress = true
                ctx.thinking = false
                ctx.speaking = false
                ctx.acceptingUserMessages = false
            case .thinking:
                ctx.captureInProgress = false
                ctx.thinking = true
                ctx.speaking = false
                ctx.acceptingUserMessages = false
            case .streaming:
                ctx.captureInProgress = false
                let hasVisibleText = self.hasVisibleGlyphs(self.streamingMessage) || self.hasVisibleGlyphs(ctx.currentMessage)
                ctx.thinking = !hasVisibleText
                ctx.speaking = hasVisibleText
                ctx.acceptingUserMessages = false
            case .unknown:
                break
            }
            ctx.activeIndicator = activeIndicator
            ctx.indicatorMessage = payload.message
            if indicator != .unknown {
                ctx.errorMessage = nil
                ctx.daemonError = false
            }
        }
    }

    /// Recoverable trigger errors → soft warning; chat → no-op; fatal → red banner.
    private func handleErrorPayload(_ payload: ErrorPayload) {
        if payload.recoverable {
            if payload.isTriggerError {
                bobeStoreLogger.warning("Trigger soft warning [\(payload.sourceLabel)]: \(payload.message)")
                self.updateState { $0.softWarning = payload.message }
            } else {
                bobeStoreLogger.warning("Recoverable chat error [\(payload.sourceLabel)]: \(payload.message)")
            }
            return
        }
        bobeStoreLogger.error("Daemon error [\(payload.sourceLabel)]: \(payload.message)")
        self.updateState { ctx in
            ctx.errorMessage = payload.message
            ctx.daemonError = true
        }
    }

    private func handleTextDelta(_ payload: TextDeltaPayload, messageId: String) {
        self.cancelConversationClear()
        if self.streamingMessageId != messageId {
            self.streamingMessage = ""
            self.streamingMessageId = messageId
            self.textDeltaFlushTask?.cancel()
            self.textDeltaFlushTask = nil
        }

        self.streamingMessage += payload.delta

        if payload.done {
            self.flushStreamingToUI(messageId: messageId)
            self.finalizeStreamingMessage()
            return
        }

        // Task existence is the dirty flag — accumulated deltas flush on timer.
        if self.textDeltaFlushTask == nil {
            self.textDeltaFlushTask = Task { @MainActor [weak self] in
                try? await Task.sleep(for: .milliseconds(StoreTiming.textDeltaFlushMilliseconds))
                guard let self, !Task.isCancelled else { return }
                self.flushStreamingToUI(messageId: messageId)
            }
        }
    }

    private func flushStreamingToUI(messageId: String) {
        self.textDeltaFlushTask?.cancel()
        self.textDeltaFlushTask = nil

        self.updateState { ctx in
            let updated = Self.updateMessage(messageId, messages: &ctx.messages) { message in
                message.content = self.streamingMessage
                message.isStreaming = true
            }
            if !updated {
                ctx.messages.append(
                    ChatMessage(
                        id: messageId, sender: .bobe, content: self.streamingMessage,
                        isStreaming: true
                    )
                )
            }

            ctx.currentMessage = self.streamingMessage
            let hasVisibleText = self.hasVisibleGlyphs(self.streamingMessage)
            ctx.thinking = !hasVisibleText
            ctx.speaking = hasVisibleText
        }
    }

    func finalizeStreamingMessage() {
        self.textDeltaFlushTask?.cancel()
        self.textDeltaFlushTask = nil

        guard let msgId = streamingMessageId else { return }

        self.updateState { ctx in
            Self.updateMessage(msgId, messages: &ctx.messages) { message in
                message.content = self.streamingMessage
                message.isStreaming = false
                message.isPending = false
            }

            ctx.lastMessage = self.streamingMessage
            ctx.currentMessage = ""
            ctx.thinking = false
            ctx.speaking = false
            ctx.activeIndicator = nil
        }

        self.streamingMessage = ""
        self.streamingMessageId = nil

        self.lastMessageTimer?.cancel()
        self.lastMessageTimer = Task { [weak self] in
            try? await Task.sleep(for: .seconds(StoreTiming.lastMessageClearSeconds))
            if !Task.isCancelled {
                self?.updateState { $0.lastMessage = nil }
            }
        }
    }

    private func handleToolCall(_ payload: AnyCodablePayload) {
        self.toolExecutionController.process(payload)
    }

    private func handleConversationClosed(_ payload: ConversationClosedPayload) {
        bobeStoreLogger.info("Conversation closed: \(payload.conversationId) (\(payload.reason), \(payload.turnCount) turns)")
        self.updateState {
            $0.thinking = false
            $0.speaking = false
            $0.activeIndicator = nil
            $0.toolExecutions = []
            $0.conversationEnding = true
        }
        self.scheduleConversationClear()
    }

    private func hasVisibleGlyphs(_ text: String) -> Bool {
        text.unicodeScalars.contains(where: {
            !$0.properties.isWhitespace && !CharacterSet.controlCharacters.contains($0)
        })
    }

    func scheduleConversationClear() {
        self.cancelConversationClear()
        self.conversationClearTask = Task { @MainActor [weak self] in
            try? await Task.sleep(for: .seconds(StoreTiming.conversationClearSeconds))
            guard let self, !Task.isCancelled else { return }
            self.clearMessages()
            self.updateState { $0.conversationEnding = false }
        }
    }

    func cancelConversationClear() {
        self.conversationClearTask?.cancel()
        self.conversationClearTask = nil
    }
}
