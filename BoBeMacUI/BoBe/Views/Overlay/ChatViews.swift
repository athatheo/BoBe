import SwiftUI
import Textual

// MARK: - Chat Stack

struct ChatStack: View {
    let messages: [ChatMessage]
    var maxViewportHeight: CGFloat = WindowSizes.heightChatViewportMax

    @State private var isExpanded = false
    @State private var expandedBubbleIds: Set<String> = []
    @State private var isNearBottom = true
    @Environment(\.theme) private var theme

    private static let maxCompactBubbles = 4
    private static let compactLineLimit = 5

    private var compactVisible: [ChatMessage] {
        Array(self.messages.suffix(Self.maxCompactBubbles))
    }

    private var hiddenCount: Int {
        max(0, self.messages.count - Self.maxCompactBubbles)
    }

    var body: some View {
        Group {
            if self.isExpanded {
                self.expandedView
            } else {
                self.compactView
            }
        }
        .frame(maxWidth: .infinity)
        .padding(.bottom, 4)
    }

    // MARK: - Compact

    /// Natural-height stack of last N short bubbles. No ScrollView — parent window resizes
    /// to fit. Read-More on a bubble removes its line cap; an overflow pill at top reveals
    /// older messages by switching to expanded mode.
    private var compactView: some View {
        VStack(spacing: 6) {
            if self.hiddenCount > 0 {
                OverflowPill(count: self.hiddenCount, isExpanded: false) {
                    self.toggleExpanded()
                }
                .transition(.opacity.combined(with: .scale(scale: 0.92)))
            }
            ForEach(self.compactVisible) { message in
                ChatBubble(
                    message: message,
                    compactLineLimit: self.effectiveCompactLineLimit(for: message),
                    onReadMore: { self.expandBubble(message.id) }
                )
                .id(message.id)
            }
        }
        .padding(.top, 2)
    }

    // MARK: - Expanded

    /// Real ScrollView capped at `maxViewportHeight`. Scrolls cleanly between whole bubbles;
    /// no mid-bubble clipping because the ScrollView's natural clip is at its frame edge.
    private var expandedView: some View {
        ScrollViewReader { proxy in
            ScrollView(showsIndicators: true) {
                LazyVStack(spacing: 6) {
                    if self.messages.count > Self.maxCompactBubbles {
                        OverflowPill(count: 0, isExpanded: true) {
                            self.toggleExpanded()
                        }
                    }
                    ForEach(self.messages) { message in
                        ChatBubble(
                            message: message,
                            compactLineLimit: nil,
                            onReadMore: nil
                        )
                        .id(message.id)
                    }
                    GeometryReader { geometry in
                        Color.clear.preference(
                            key: ChatBottomPreferenceKey.self,
                            value: geometry.frame(in: .named("chatScroll")).maxY
                        )
                    }
                    .frame(height: 1)
                }
                .padding(.vertical, 2)
            }
            .coordinateSpace(name: "chatScroll")
            .onPreferenceChange(ChatBottomPreferenceKey.self) { bottomY in
                self.isNearBottom = bottomY <= self.maxViewportHeight + 48
            }
            .accessibilityIdentifier("overlay.chat.scroll")
            .frame(maxHeight: self.maxViewportHeight, alignment: .bottom)
            .scrollBounceBehavior(.basedOnSize)
            // Let bubble shadows escape the clip rect.
            .scrollClipDisabled()
            .defaultScrollAnchor(.bottom)
            .onAppear {
                self.scrollToBottom(proxy, animated: false)
            }
            .onChange(of: self.messages.last?.id) { _, _ in
                if self.isNearBottom { self.scrollToBottom(proxy, animated: true) }
            }
            .onChange(of: self.messages.last?.content) { _, _ in
                if self.isNearBottom { self.scrollToBottom(proxy, animated: true) }
            }
        }
    }

    private func scrollToBottom(_ proxy: ScrollViewProxy, animated: Bool) {
        guard let last = self.messages.last else { return }
        if animated {
            withAnimation(OverlayMotionRuntime.reduceMotion ? nil : .linear(duration: 0.12)) {
                proxy.scrollTo(last.id, anchor: .bottom)
            }
        } else {
            proxy.scrollTo(last.id, anchor: .bottom)
        }
    }

    private func toggleExpanded() {
        // No withAnimation: window resize fights content animation, causing shake.
        self.isExpanded.toggle()
        if !self.isExpanded {
            self.expandedBubbleIds.removeAll()
        }
    }

    private func expandBubble(_ id: String) {
        _ = withAnimation(OverlayMotionRuntime.animation(for: .chatTransition)) {
            self.expandedBubbleIds.insert(id)
        }
    }

    private func effectiveCompactLineLimit(for message: ChatMessage) -> Int? {
        if self.expandedBubbleIds.contains(message.id) {
            return nil
        }
        return Self.compactLineLimit
    }
}

private struct ChatBottomPreferenceKey: PreferenceKey {
    static let defaultValue: CGFloat = 0

    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) {
        value = nextValue()
    }
}

// MARK: - Overflow Pill

private struct OverflowPill: View {
    let count: Int
    let isExpanded: Bool
    let action: () -> Void

    @Environment(\.theme) private var theme

    var body: some View {
        Button(action: self.action) {
            HStack(spacing: 5) {
                Image(systemName: self.isExpanded ? "chevron.down" : "chevron.up")
                    .font(.system(size: 7, weight: .bold))

                Text(self.label)
                    .bobeTextStyle(.chatMeta)
            }
            .foregroundStyle(self.theme.colors.textMuted)
            .padding(.horizontal, 12)
            .padding(.vertical, 5)
            .background(
                Capsule()
                    .fill(self.theme.colors.surface.opacity(0.92))
                    .overlay(
                        Capsule()
                            .strokeBorder(self.theme.colors.border.opacity(0.5), lineWidth: 0.5)
                    )
                    .shadow(color: self.theme.colors.text.opacity(0.05), radius: 3, y: 1)
            )
        }
        .buttonStyle(.plain)
        .contentShape(Capsule())
        .frame(maxWidth: .infinity, alignment: .center)
        .accessibilityLabel(self.label)
        .accessibilityIdentifier(self.isExpanded ? "overlay.chat.collapse" : "overlay.chat.expand")
    }

    private var label: String {
        if self.isExpanded {
            return L10n.tr("overlay.chat.action.collapse")
        }
        return L10n.tr("overlay.chat.hidden_messages_format", self.count)
    }
}

// MARK: - Chat Bubble

struct ChatBubble: View {
    let message: ChatMessage
    var compactLineLimit: Int?
    var onReadMore: (() -> Void)?

    @Environment(\.theme) private var theme
    /// Cached inline-markdown parse for finalized (non-streaming) bubbles.
    /// `AttributedString(markdown:)` runs the full parser over the whole
    /// string each time, so on a streaming reply with 50 chunks the naive
    /// pattern parsed N=1, N=2, … N=50 — O(N²) work over a turn. We now
    /// skip the parse entirely while `isStreaming` is true (plain `Text`
    /// renders the growing content unparsed) and parse exactly once when
    /// the stream finishes.
    @State private var renderedContent: AttributedString = AttributedString()

    private var isUser: Bool {
        self.message.sender == .user
    }

    private var isPending: Bool {
        self.message.isPending
    }

    private var accentColor: Color {
        self.isUser ? self.theme.colors.secondary : self.theme.colors.primary
    }

    private var showReadMore: Bool {
        guard let limit = self.compactLineLimit, self.onReadMore != nil else { return false }
        return Self.isLikelyTruncated(self.message.content, lineLimit: limit)
    }

    /// Inline markdown (bold/italic/code/links) only; block syntax stays as text so paragraphs and lists keep their layout.
    private static func parseInline(_ content: String) -> AttributedString {
        let options = AttributedString.MarkdownParsingOptions(
            interpretedSyntax: .inlineOnlyPreservingWhitespace
        )
        if let attr = try? AttributedString(markdown: content, options: options) {
            return attr
        }
        return AttributedString(content)
    }

    var body: some View {
        HStack {
            if self.isUser {
                Spacer(minLength: 0)
            }

            VStack(spacing: 0) {
                if self.message.isStreaming {
                    Rectangle()
                        .fill(self.accentColor)
                        .frame(height: 3)
                        .transition(.opacity)
                }

                VStack(alignment: .leading, spacing: 0) {
                    HStack(spacing: 0) {
                        Text(self.isUser ? L10n.tr("overlay.chat.sender.you") : L10n.tr("overlay.chat.sender.bobe"))
                            .bobeTextStyle(.chatSender)
                            .tracking(0.8)
                            .textCase(.uppercase)
                            .foregroundStyle(self.accentColor)
                        if self.isPending {
                            Text(L10n.tr("overlay.chat.pending_suffix"))
                                .bobeTextStyle(.chatPending)
                                .italic()
                                .foregroundStyle(self.theme.colors.textMuted)
                        }
                    }
                    .padding(.bottom, 2)

                    HStack(spacing: 0) {
                        // Three render paths:
                        //  • Streaming  → plain `Text` over the raw string. No
                        //    markdown parse per chunk; the user sees the bold
                        //    asterisks etc. for a moment, then once the stream
                        //    ends we flip to the parsed view. Trade-off worth
                        //    it: parsing every chunk was O(N²) on the reply.
                        //  • Expanded   → full block-level markdown.
                        //  • Compact    → cached inline-parsed AttributedString
                        //    so truncation works (StructuredText doesn't
                        //    support `lineLimit`).
                        if self.message.isStreaming {
                            Text(self.message.content)
                                .bobeTextStyle(.chatBody)
                                .lineSpacing(2)
                                .foregroundStyle(self.theme.colors.text)
                                .fixedSize(horizontal: false, vertical: true)
                        } else if self.compactLineLimit == nil {
                            StructuredText(markdown: self.message.content)
                                .textual.structuredTextStyle(.gitHub)
                                .textual.textSelection(.disabled)
                                .foregroundStyle(self.theme.colors.text)
                                .fixedSize(horizontal: false, vertical: true)
                        } else {
                            Text(self.renderedContent)
                                .bobeTextStyle(.chatBody)
                                .lineSpacing(2)
                                .foregroundStyle(self.theme.colors.text)
                                .lineLimit(self.compactLineLimit)
                        }

                        if self.message.isStreaming {
                            BlinkingCursor(color: self.theme.colors.primary)
                        }
                    }

                    if self.showReadMore {
                        Button {
                            self.onReadMore?()
                        } label: {
                            Text(L10n.tr("overlay.chat.action.read_more"))
                                .bobeTextStyle(.chatMeta)
                                .foregroundStyle(self.theme.colors.primary.opacity(0.7))
                        }
                        .buttonStyle(.plain)
                        .padding(.top, 3)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 12)
                .padding(.top, 8)
                .padding(.bottom, 10)
            }
            .background(
                self.isPending
                    ? self.theme.colors.surface
                    : self.theme.colors.background
            )
            .clipShape(RoundedRectangle(cornerRadius: 16))
            .overlay(
                RoundedRectangle(cornerRadius: 16)
                    .strokeBorder(
                        self.theme.colors.border,
                        style: StrokeStyle(
                            lineWidth: 1.5,
                            dash: self.isPending ? [4, 3] : []
                        )
                    )
            )
            .shadow(color: self.theme.colors.text.opacity(0.07), radius: 4, y: 2)
            .frame(maxWidth: 440, alignment: self.isUser ? .trailing : .leading)
            .transition(
                OverlayMotionRuntime.reduceMotion
                    ? .opacity
                    : .asymmetric(
                        insertion: .move(edge: .bottom).combined(with: .opacity).combined(with: .scale(scale: 0.95)),
                        removal: .opacity
                    )
            )

            if !self.isUser {
                Spacer(minLength: 0)
            }
        }
        // Parse only when we actually need the AttributedString — i.e.
        // compact-mode rendering after streaming finishes. Streaming
        // bubbles use plain Text and never touch the parser; once the
        // stream ends, `.task(id:)` fires once with the final content and
        // we cache it for the lifetime of the bubble.
        .task(id: self.parseIdentity) {
            guard !self.message.isStreaming, self.compactLineLimit != nil else { return }
            self.renderedContent = Self.parseInline(self.message.content)
        }
    }

    /// Composite key that changes exactly when we'd need a fresh parse.
    /// Streaming → no parse (handled by the early-return above). Finalized
    /// content change → re-parse. Compact-mode entry → parse on first task.
    private var parseIdentity: String {
        // `isStreaming` is captured so the task runs again on stream-finish
        // — otherwise the bubble would stay on the streaming plain-Text path.
        "\(self.message.isStreaming ? "S" : "F")|\(self.message.content)"
    }

    private static func isLikelyTruncated(_ content: String, lineLimit: Int) -> Bool {
        content.filter(\.isNewline).count >= lineLimit || content.count > lineLimit * 50
    }
}

struct BlinkingCursor: View {
    let color: Color

    @State private var visible = true

    var body: some View {
        Text("|")
            .bobeTextStyle(.chatBody)
            .fontWeight(.semibold)
            .foregroundStyle(self.color)
            .opacity(self.visible ? 1 : 0)
            .task {
                guard OverlayMotionRuntime.shouldAnimate else { return }
                while !Task.isCancelled {
                    try? await Task.sleep(for: .seconds(0.5))
                    self.visible.toggle()
                }
            }
    }
}

// MARK: - Previews

#if !SPM_BUILD
    #Preview("Chat Bubble - User") {
        ChatBubble(message: ChatMessage(sender: .user, content: "Hello BoBe, how are you?"))
            .environment(\.theme, allThemes[0])
            .padding()
            .frame(width: 500)
    }

    #Preview("Chat Bubble - BoBe") {
        ChatBubble(
            message: ChatMessage(sender: .bobe, content: "I'm doing great! I've been observing your workflow and noticed some interesting patterns.")
        )
        .environment(\.theme, allThemes[0])
        .padding()
        .frame(width: 500)
    }

    #Preview("Chat Bubble - Markdown") {
        ChatBubble(
            message: ChatMessage(sender: .bobe, content: """
            **Code & Development**
            - Write, refactor, debug code across any language
            - Explore unfamiliar codebases and explain how things work
            - Run tests, builds, linters

            **Git & Collaboration**
            - Commits, branches, PRs via `gh` or `tea`
            """)
        )
        .environment(\.theme, allThemes[0])
        .padding()
        .frame(width: 500)
    }

    #Preview("Chat Bubble - Streaming") {
        ChatBubble(message: ChatMessage(sender: .bobe, content: "Thinking about this", isStreaming: true))
            .environment(\.theme, allThemes[0])
            .padding()
            .frame(width: 500)
    }

    #Preview("Chat Stack") {
        ChatStack(messages: [
            ChatMessage(sender: .bobe, content: "Hey! I noticed you've been working on the settings panel."),
            ChatMessage(sender: .user, content: "Yeah, I'm trying to get the theme picker working."),
            ChatMessage(sender: .bobe, content: "I can see that! The colors are looking great so far."),
            ChatMessage(sender: .user, content: "Thanks! Any suggestions?"),
            ChatMessage(sender: .bobe, content: "You might want to add a preview for the avatar in each theme card."),
        ])
        .environment(\.theme, allThemes[0])
        .frame(width: 540, height: 400)
        .background(allThemes[0].colors.background)
    }
#endif
