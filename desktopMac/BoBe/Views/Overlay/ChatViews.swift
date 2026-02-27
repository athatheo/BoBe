import SwiftUI

/// Stack of chat message bubbles. Based on ChatStack: collapsed shows last 2, expanded shows all.
struct ChatStack: View {
    let messages: [ChatMessage]

    @State private var isExpanded = false
    @Environment(\.theme) private var theme

    private var visibleMessages: [ChatMessage] {
        if isExpanded { return messages }
        return Array(messages.suffix(4))
    }

    private var hiddenCount: Int {
        max(0, messages.count - 4)
    }

    private var shouldShowToggle: Bool {
        isExpanded || hiddenCount > 0
    }

    var body: some View {
        VStack(spacing: 0) {
            // Messages
            ScrollViewReader { proxy in
                ScrollView(showsIndicators: false) {
                    VStack(spacing: 8) {
                        ForEach(visibleMessages) { message in
                            ChatBubble(message: message)
                                .id(message.id)
                        }
                    }
                    .padding(.top, isExpanded ? 4 : 0)
                }
                .onChange(of: messages.count) { _, _ in
                    if let last = visibleMessages.last {
                        withAnimation {
                            proxy.scrollTo(last.id, anchor: .bottom)
                        }
                    }
                }
                .frame(maxHeight: isExpanded ? 520 : 420)
            }

            // Expand/collapse button (chat-expand-button)
            if shouldShowToggle {
                Button {
                    withAnimation(.spring(duration: 0.3, bounce: 0.1)) {
                        isExpanded.toggle()
                    }
                } label: {
                    HStack(spacing: 4) {
                        Image(systemName: isExpanded ? "chevron.down" : "chevron.up")
                            .font(.system(size: 8))
                        Text(isExpanded ? "collapse" : "+\(hiddenCount) more")
                            .font(.system(size: 10, weight: .medium))
                    }
                    .foregroundStyle(theme.colors.textMuted)
                    .padding(.horizontal, 10)
                    .padding(.vertical, 4)
                    .background(
                        RoundedRectangle(cornerRadius: 12)
                            .fill(theme.colors.background)
                            .overlay(
                                RoundedRectangle(cornerRadius: 12)
                                    .stroke(theme.colors.border, lineWidth: 1)
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

/// Individual chat message bubble — pixel-perfect match to CSS .chat-bubble
struct ChatBubble: View {
    let message: ChatMessage

    @Environment(\.theme) private var theme

    private var isUser: Bool { message.sender == .user }
    private var isPending: Bool { message.isPending }
    private var accentColor: Color {
        isUser ? theme.colors.secondary : theme.colors.primary
    }

    var body: some View {
        HStack {
            if isUser { Spacer(minLength: 0) }

            // Bubble container
            VStack(spacing: 0) {
                // 3px accent bar at top
                Rectangle()
                    .fill(accentColor)
                    .frame(height: 3)

                // Content area (padding: 8px 12px 10px)
                VStack(alignment: .leading, spacing: 0) {
                    // Sender label (9px semibold uppercase)
                    HStack(spacing: 0) {
                        Text(isUser ? "you" : "bobe")
                            .font(.system(size: 9, weight: .semibold))
                            .tracking(0.8)
                            .textCase(.uppercase)
                            .foregroundStyle(accentColor)
                        if isPending {
                            Text(" - sending...")
                                .font(.system(size: 8))
                                .italic()
                                .foregroundStyle(theme.colors.textMuted)
                        }
                    }
                    .padding(.bottom, 2)

                    // Message text (12px)
                    HStack(spacing: 0) {
                        Text(message.content)
                            .font(.system(size: 12))
                            .lineSpacing(2)
                            .foregroundStyle(theme.colors.text)
                            .fixedSize(horizontal: false, vertical: true)

                        if message.isStreaming {
                            BlinkingCursor(color: theme.colors.primary)
                        }
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 12)
                .padding(.top, 8)
                .padding(.bottom, 10)
            }
            .background(
                isPending ? theme.colors.border : theme.colors.background
            )
            .clipShape(RoundedRectangle(cornerRadius: 16))
            .overlay(
                RoundedRectangle(cornerRadius: 16)
                    .stroke(theme.colors.border, lineWidth: 1.5)
            )
            .shadow(color: Color.black.opacity(0.06), radius: 4, y: 2)
            .opacity(isPending ? 0.5 : 1)
            .frame(maxWidth: isUser ? 410 : 460, alignment: isUser ? .trailing : .leading)
            .transition(.asymmetric(
                insertion: .move(edge: .bottom).combined(with: .opacity).combined(with: .scale(scale: 0.95)),
                removal: .opacity
            ))

            if !isUser { Spacer(minLength: 0) }
        }
    }
}

/// Blinking cursor shown during streaming
struct BlinkingCursor: View {
    let color: Color

    @State private var visible = true

    var body: some View {
        Text("|")
            .font(.system(size: 12, weight: .semibold))
            .foregroundStyle(color)
            .opacity(visible ? 1 : 0)
            .task {
                while !Task.isCancelled {
                    try? await Task.sleep(for: .seconds(0.3))
                    visible.toggle()
                }
            }
    }
}
