import SwiftUI

struct ChatStack: View {
    let messages: [ChatMessage]
    var maxViewportHeight: CGFloat = WindowSizes.heightChatViewportMax

    @State private var isExpanded = false
    @Environment(\.theme) private var theme

    private var visibleMessages: [ChatMessage] {
        if self.isExpanded { return self.messages }
        return Array(self.messages.suffix(4))
    }

    private var hiddenCount: Int {
        max(0, self.messages.count - 4)
    }

    private var shouldShowToggle: Bool {
        self.isExpanded || self.hiddenCount > 0
    }

    var body: some View {
        VStack(spacing: 0) {
            ScrollViewReader { proxy in
                ScrollView(showsIndicators: false) {
                    VStack(spacing: 8) {
                        if !self.isExpanded, self.hiddenCount > 0 {
                            HiddenMessagesAffordance(count: self.hiddenCount)
                                .transition(.opacity)
                        }

                        ForEach(self.visibleMessages) { message in
                            ChatBubble(message: message)
                                .id(message.id)
                        }
                    }
                    .padding(.top, self.isExpanded ? 4 : 2)
                }
                .overlay(alignment: .top) {
                    if !self.isExpanded, self.hiddenCount > 0 {
                        LinearGradient(
                            colors: [self.theme.colors.background.opacity(0.95), self.theme.colors.background.opacity(0)],
                            startPoint: .top,
                            endPoint: .bottom
                        )
                        .frame(height: 22)
                        .allowsHitTesting(false)
                    }
                }
                .onAppear {
                    if let last = messages.last {
                        proxy.scrollTo(last.id, anchor: .bottom)
                    }
                }
                .onChange(of: self.messages.last?.id) { _, _ in
                    if let last = messages.last {
                        withAnimation(OverlayMotionRuntime.animation(for: .chatTransition)) {
                            proxy.scrollTo(last.id, anchor: .bottom)
                        }
                    }
                }
                .onChange(of: self.messages.last?.content) { _, _ in
                    if let last = messages.last {
                        withAnimation(.linear(duration: 0.12)) {
                            proxy.scrollTo(last.id, anchor: .bottom)
                        }
                    }
                }
                .frame(maxHeight: self.maxViewportHeight)
            }

            if self.shouldShowToggle {
                Button {
                    withAnimation(OverlayMotionRuntime.animation(for: .chatTransition)) {
                        self.isExpanded.toggle()
                    }
                } label: {
                    HStack(spacing: 4) {
                        Image(systemName: self.isExpanded ? "chevron.down" : "chevron.up")
                            .font(.system(size: 8))
                        Text(
                            self.isExpanded
                                ? L10n.tr("overlay.chat.action.collapse")
                                : L10n.tr("overlay.chat.hidden_messages_format", self.hiddenCount)
                        )
                            .font(.system(size: 10, weight: .medium))
                    }
                    .foregroundStyle(self.theme.colors.textMuted)
                    .padding(.horizontal, 10)
                    .padding(.vertical, 4)
                    .background(
                        RoundedRectangle(cornerRadius: 12)
                            .fill(self.theme.colors.background)
                            .overlay(
                                RoundedRectangle(cornerRadius: 12)
                                    .stroke(self.theme.colors.border, lineWidth: 1)
                            )
                    )
                }
                .buttonStyle(.plain)
                .frame(maxWidth: .infinity, alignment: .center)
                .padding(.top, 8)
            }
        }
        .frame(maxWidth: .infinity)
        .padding(.bottom, 4)
    }
}

private struct HiddenMessagesAffordance: View {
    let count: Int
    @Environment(\.theme) private var theme

    var body: some View {
        HStack(spacing: 4) {
            Image(systemName: "ellipsis")
                .font(.system(size: 9, weight: .semibold))
            Text(L10n.tr("overlay.chat.hidden_messages_format", self.count))
                .font(.system(size: 10, weight: .medium))
        }
        .foregroundStyle(self.theme.colors.textMuted)
        .padding(.horizontal, 12)
        .padding(.vertical, 5)
        .background(
            Capsule()
                .fill(self.theme.colors.background.opacity(0.86))
                .overlay(Capsule().stroke(self.theme.colors.border, lineWidth: 1))
        )
        .frame(maxWidth: .infinity, alignment: .center)
    }
}

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
        HStack {
            if self.isUser { Spacer(minLength: 0) }

            VStack(spacing: 0) {
                Rectangle()
                    .fill(self.accentColor)
                    .frame(height: 3)

                VStack(alignment: .leading, spacing: 0) {
                    HStack(spacing: 0) {
                        Text(self.isUser ? L10n.tr("overlay.chat.sender.you") : L10n.tr("overlay.chat.sender.bobe"))
                            .font(.system(size: 9, weight: .semibold))
                            .tracking(0.8)
                            .textCase(.uppercase)
                            .foregroundStyle(self.accentColor)
                        if self.isPending {
                            Text(L10n.tr("overlay.chat.pending_suffix"))
                                .font(.system(size: 8))
                                .italic()
                                .foregroundStyle(self.theme.colors.textMuted)
                        }
                    }
                    .padding(.bottom, 2)

                    HStack(spacing: 0) {
                        Text(self.message.content)
                            .font(.system(size: 12))
                            .lineSpacing(2)
                            .foregroundStyle(self.theme.colors.text)
                            .fixedSize(horizontal: false, vertical: true)

                        if self.message.isStreaming {
                            BlinkingCursor(color: self.theme.colors.primary)
                        }
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 12)
                .padding(.top, 8)
                .padding(.bottom, 10)
            }
            .background(
                self.isPending ? self.theme.colors.border : self.theme.colors.background
            )
            .clipShape(RoundedRectangle(cornerRadius: 16))
            .overlay(
                RoundedRectangle(cornerRadius: 16)
                    .stroke(self.theme.colors.border, lineWidth: 1.5)
            )
            .shadow(color: Color.black.opacity(0.06), radius: 4, y: 2)
            .opacity(self.isPending ? 0.5 : 1)
            .frame(maxWidth: self.isUser ? 410 : 460, alignment: self.isUser ? .trailing : .leading)
            .transition(
                .asymmetric(
                    insertion: .move(edge: .bottom).combined(with: .opacity).combined(with: .scale(scale: 0.95)),
                    removal: .opacity
                )
            )

            if !self.isUser { Spacer(minLength: 0) }
        }
    }
}

struct BlinkingCursor: View {
    let color: Color

    @State private var visible = true

    var body: some View {
        Text("|")
            .font(.system(size: 12, weight: .semibold))
            .foregroundStyle(self.color)
            .opacity(self.visible ? 1 : 0)
            .task {
                while !Task.isCancelled {
                    try? await Task.sleep(for: .seconds(0.3))
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
    .frame(width: 500, height: 400)
    .background(allThemes[0].colors.background)
}
#endif
