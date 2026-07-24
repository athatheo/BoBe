import SwiftUI
import Textual

// MARK: - Chat Stack

struct ChatStack: View {
    let messages: [ChatMessage]
    var maxViewportHeight: CGFloat = WindowSizes.heightChatViewportMax

    @State private var isNearBottom = true
    @State private var isAtTop = true
    @Environment(\.theme) private var theme

    private var displayableMessages: [ChatMessage] {
        self.messages.filter(\.belongsInConversationTrace)
    }

    var body: some View {
        self.readerView
        .frame(maxWidth: .infinity)
        .padding(.horizontal, 12)
        .padding(.vertical, 5)
        .background(
            RoundedRectangle(cornerRadius: 14)
                .fill(self.theme.colors.surface)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 14)
                .stroke(self.theme.colors.border.opacity(0.75), lineWidth: 1)
        )
        .shadow(color: self.theme.colors.text.opacity(0.07), radius: 5, y: 2)
        .padding(.bottom, 4)
    }

    private var readerView: some View {
        ScrollViewReader { proxy in
            ScrollView(showsIndicators: true) {
                LazyVStack(spacing: 0) {
                    ForEach(self.displayableMessages) { message in
                        ChatBubble(message: message)
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
            .onScrollGeometryChange(for: Bool.self) { geometry in
                geometry.visibleRect.minY <= 1
            } action: { _, isAtTop in
                self.isAtTop = isAtTop
            }
            .accessibilityIdentifier("overlay.chat.scroll")
            .frame(maxHeight: self.maxViewportHeight, alignment: .bottom)
            .scrollBounceBehavior(.basedOnSize)
            .defaultScrollAnchor(.bottom)
            .overlay(alignment: .top) {
                if !self.isAtTop {
                    LinearGradient(
                        stops: [
                            .init(color: self.theme.colors.surface, location: 0),
                            .init(color: self.theme.colors.surface, location: 0.35),
                            .init(color: self.theme.colors.surface.opacity(0), location: 1),
                        ],
                        startPoint: .top,
                        endPoint: .bottom
                    )
                    .frame(height: 12)
                    .allowsHitTesting(false)
                    .accessibilityHidden(true)
                }
            }
            .onAppear {
                self.scrollToBottom(proxy, animated: false)
            }
            .onChange(of: self.displayableMessages.last?.id) { _, _ in
                if self.isNearBottom { self.scrollToBottom(proxy, animated: true) }
            }
            .onChange(of: self.displayableMessages.last?.content) { _, _ in
                if self.isNearBottom { self.scrollToBottom(proxy, animated: true) }
            }
        }
    }

    private func scrollToBottom(_ proxy: ScrollViewProxy, animated: Bool) {
        guard let last = self.displayableMessages.last else { return }
        if animated {
            withAnimation(OverlayMotionRuntime.reduceMotion ? nil : .linear(duration: 0.12)) {
                proxy.scrollTo(last.id, anchor: .bottom)
            }
        } else {
            proxy.scrollTo(last.id, anchor: .bottom)
        }
    }
}

private struct ChatBottomPreferenceKey: PreferenceKey {
    static let defaultValue: CGFloat = 0

    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) {
        value = nextValue()
    }
}

// MARK: - Chat Bubble

struct ChatBubble: View {
    let message: ChatMessage

    @Environment(\.theme) private var theme
    private var isUser: Bool {
        self.message.sender == .user
    }

    private var isPending: Bool {
        self.message.isPending
    }

    private var accentColor: Color {
        self.isUser ? self.theme.colors.secondary : self.theme.colors.primary
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
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
                if !self.isUser, !self.message.isComplete {
                    Text(L10n.tr("overlay.chat.interrupted_suffix"))
                        .bobeTextStyle(.chatPending)
                        .foregroundStyle(self.theme.colors.error)
                }
            }
            .padding(.bottom, 1)

            HStack(spacing: 0) {
                if self.message.isStreaming {
                    Text(self.message.content)
                        .bobeTextStyle(.chatBody)
                        .lineSpacing(2)
                        .foregroundStyle(self.theme.colors.text)
                        .fixedSize(horizontal: false, vertical: true)
                } else {
                    StructuredText(markdown: self.message.content)
                        .textual.structuredTextStyle(.gitHub)
                        .textual.textSelection(.enabled)
                        .foregroundStyle(self.theme.colors.text)
                        .fixedSize(horizontal: false, vertical: true)
                }

                if self.message.isStreaming {
                    StreamingCursor(color: self.theme.colors.primary)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 4)
        .padding(.vertical, 9)
        .overlay(alignment: .bottom) {
            Rectangle()
                .fill(self.theme.colors.border.opacity(0.45))
                .frame(height: 0.5)
        }
        .transition(.opacity)
    }
}

struct StreamingCursor: View {
    let color: Color

    var body: some View {
        Text("|")
            .bobeTextStyle(.chatBody)
            .fontWeight(.semibold)
            .foregroundStyle(self.color)
            .accessibilityHidden(true)
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
