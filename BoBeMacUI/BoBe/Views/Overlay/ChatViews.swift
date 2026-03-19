import SwiftUI

// MARK: - Visual Segment

/// A visual unit within the chat — either a complete short message or
/// one paragraph of a multi-paragraph response. Keeps the data model
/// clean (1 SSE message = 1 `ChatMessage`) while letting the UI break
/// long responses into digestible pills.
private struct ChatSegment: Identifiable {
    var id: String { self.message.id }
    let message: ChatMessage
    let showSender: Bool
    let isContinuation: Bool
}

// MARK: - Chat Stack

struct ChatStack: View {
    let messages: [ChatMessage]
    var maxViewportHeight: CGFloat = WindowSizes.heightChatViewportMax

    @State private var isExpanded = false
    /// Per-bubble expand — lets users reveal truncated text without expanding
    /// the entire chat history.
    @State private var expandedBubbleIds: Set<String> = []
    /// IDs of messages that just finished streaming. While present the message
    /// stays as a single bubble; after 300 ms it splits into paragraph pills
    /// so the transition feels like a natural "unfold" instead of a jarring cut.
    @State private var deferSplitIds: Set<String> = []
    @Environment(\.theme) private var theme

    private static let maxCompactSegments = 4
    private static let compactLineLimit = 5

    // MARK: - Segment Computation

    private var allSegments: [ChatSegment] {
        self.messages.flatMap { msg in
            if self.deferSplitIds.contains(msg.id) {
                return [ChatSegment(message: msg, showSender: true, isContinuation: false)]
            }
            return Self.splitMessage(msg)
        }
    }

    private var visibleSegments: [ChatSegment] {
        if self.isExpanded { return self.allSegments }
        return Array(self.allSegments.suffix(Self.maxCompactSegments))
    }

    /// Only counts segments hidden above the visible window.
    /// Per-bubble truncation is handled separately by "read more" links.
    private var hiddenCount: Int {
        if self.isExpanded { return 0 }
        return max(0, self.allSegments.count - Self.maxCompactSegments)
    }

    var body: some View {
        VStack(spacing: 0) {
            ScrollViewReader { proxy in
                ScrollView(showsIndicators: self.isExpanded) {
                    VStack(spacing: 6) {
                        if !self.isExpanded, self.hiddenCount > 0 {
                            OverflowPill(count: self.hiddenCount, isExpanded: false) {
                                self.toggleExpanded()
                            }
                            .transition(.opacity.combined(with: .scale(scale: 0.92)))
                        }

                        if self.isExpanded, self.allSegments.count > Self.maxCompactSegments {
                            OverflowPill(count: 0, isExpanded: true) {
                                self.toggleExpanded()
                            }
                            .transition(.opacity)
                        }

                        ForEach(self.visibleSegments) { segment in
                            ChatBubble(
                                message: segment.message,
                                showSender: segment.showSender,
                                compactLineLimit: self.effectiveLineLimit(for: segment),
                                isContinuation: segment.isContinuation,
                                onReadMore: self.isExpanded ? nil : { self.expandBubble(segment.id) }
                            )
                            .id(segment.id)
                        }
                    }
                    .padding(.top, 2)
                    .padding(.bottom, 2)
                }
                // Shrink-to-fit: ScrollView takes only the space its content
                // needs, up to maxViewportHeight. Without fixedSize the
                // ScrollView is greedy and creates a huge gap above messages.
                .defaultScrollAnchor(.bottom)
                .fixedSize(horizontal: false, vertical: true)
                .frame(maxHeight: self.maxViewportHeight, alignment: .bottom)
                // Disable bounce when content fits — prevents rubberbanding
                // on short conversations.
                .scrollBounceBehavior(.basedOnSize)
                // Allow bubble shadows to render outside the scroll clip rect.
                .scrollClipDisabled()
                .onAppear {
                    self.scrollToBottom(proxy)
                }
                .onChange(of: self.messages.last?.id) { _, _ in
                    withAnimation(OverlayMotionRuntime.animation(for: .chatTransition)) {
                        self.scrollToBottom(proxy)
                    }
                }
                .onChange(of: self.messages.last?.content) { _, _ in
                    withAnimation(OverlayMotionRuntime.reduceMotion ? nil : .linear(duration: 0.12)) {
                        self.scrollToBottom(proxy)
                    }
                }
                .onChange(of: self.messages.last?.isStreaming) { old, new in
                    if old == true, new == false, let id = self.messages.last?.id {
                        self.scheduleDeferredSplit(for: id)
                    }
                }
            }
        }
        .frame(maxWidth: .infinity)
        .padding(.bottom, 4)
    }

    // MARK: - Actions

    private func scrollToBottom(_ proxy: ScrollViewProxy) {
        if let last = self.visibleSegments.last {
            proxy.scrollTo(last.id, anchor: .bottom)
        }
    }

    private func toggleExpanded() {
        // No withAnimation — the window resize (triggered by GeometryReader
        // preference key) fights with content animation, causing shake.
        // Individual view transitions still fire from conditional presence.
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

    private func effectiveLineLimit(for segment: ChatSegment) -> Int? {
        if self.isExpanded { return nil }
        if self.expandedBubbleIds.contains(segment.id) { return nil }
        return Self.compactLineLimit
    }

    /// Keeps a just-finalized message as a single bubble for 300 ms, then
    /// removes the deferral so paragraph splitting kicks in with animation.
    private func scheduleDeferredSplit(for id: String) {
        self.deferSplitIds.insert(id)
        Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(300))
            guard !Task.isCancelled else { return }
            _ = withAnimation(OverlayMotionRuntime.animation(for: .chatTransition)) {
                self.deferSplitIds.remove(id)
            }
        }
    }

    // MARK: - Segment Helpers

    private static func splitMessage(_ message: ChatMessage) -> [ChatSegment] {
        guard !message.isStreaming else {
            return [ChatSegment(message: message, showSender: true, isContinuation: false)]
        }

        let paragraphs = message.content
            .components(separatedBy: "\n\n")
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }

        guard paragraphs.count > 1 else {
            return [ChatSegment(message: message, showSender: true, isContinuation: false)]
        }

        return paragraphs.enumerated().map { index, paragraph in
            ChatSegment(
                message: ChatMessage(
                    // First paragraph keeps the original ID so SwiftUI animates
                    // it as an update rather than a remove+insert.
                    id: index == 0 ? message.id : "\(message.id)-p\(index)",
                    sender: message.sender,
                    content: paragraph,
                    timestamp: message.timestamp,
                    isPending: message.isPending
                ),
                showSender: index == 0,
                isContinuation: index > 0
            )
        }
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
                    .shadow(color: Color.black.opacity(0.04), radius: 3, y: 1)
            )
        }
        .buttonStyle(.plain)
        .contentShape(Capsule())
        .frame(maxWidth: .infinity, alignment: .center)
        .accessibilityLabel(self.label)
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
    var showSender: Bool = true
    var compactLineLimit: Int?
    var isContinuation: Bool = false
    var onReadMore: (() -> Void)?

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

    private var cornerRadius: CGFloat {
        self.isContinuation ? 12 : 16
    }

    private var showReadMore: Bool {
        guard let limit = self.compactLineLimit, self.onReadMore != nil else { return false }
        return Self.isLikelyTruncated(self.message.content, lineLimit: limit)
    }

    var body: some View {
        HStack {
            if self.isUser { Spacer(minLength: 0) }

            VStack(spacing: 0) {
                if !self.isContinuation {
                    Rectangle()
                        .fill(self.accentColor)
                        .frame(height: 3)
                }

                VStack(alignment: .leading, spacing: 0) {
                    if self.showSender {
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
                    }

                    HStack(spacing: 0) {
                        Text(self.message.content)
                            .bobeTextStyle(.chatBody)
                            .lineSpacing(2)
                            .foregroundStyle(self.theme.colors.text)
                            .lineLimit(self.compactLineLimit)
                            .fixedSize(horizontal: false, vertical: self.compactLineLimit == nil)

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
                .padding(.top, self.isContinuation ? 6 : 8)
                .padding(.bottom, 10)
            }
            .background(
                self.isPending ? self.theme.colors.border : self.theme.colors.background
            )
            .clipShape(RoundedRectangle(cornerRadius: self.cornerRadius))
            .overlay(
                RoundedRectangle(cornerRadius: self.cornerRadius)
                    .stroke(self.theme.colors.border, lineWidth: self.isContinuation ? 1 : 1.5)
            )
            .shadow(
                color: Color.black.opacity(self.isContinuation ? 0.03 : 0.06),
                radius: self.isContinuation ? 2 : 4,
                y: self.isContinuation ? 1 : 2
            )
            .opacity(self.isPending ? 0.5 : 1)
            .frame(maxWidth: self.isUser ? 410 : 460, alignment: self.isUser ? .trailing : .leading)
            .transition(
                OverlayMotionRuntime.reduceMotion
                    ? .opacity
                    : .asymmetric(
                        insertion: .move(edge: .bottom).combined(with: .opacity).combined(with: .scale(scale: 0.95)),
                        removal: .opacity
                    )
            )

            if !self.isUser { Spacer(minLength: 0) }
        }
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
            .font(.system(size: 12, weight: .semibold))
            .foregroundStyle(self.color)
            .opacity(self.visible ? 1 : 0)
            .task {
                guard OverlayMotionRuntime.shouldAnimate else { return }
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

#Preview("Chat Bubble - Continuation") {
    VStack(spacing: 3) {
        ChatBubble(
            message: ChatMessage(sender: .bobe, content: "First paragraph with the main idea.")
        )
        ChatBubble(
            message: ChatMessage(sender: .bobe, content: "Second paragraph with more details about the topic."),
            showSender: false,
            isContinuation: true
        )
    }
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

#Preview("Chat Stack - Long Response") {
    ChatStack(messages: [
        ChatMessage(sender: .user, content: "Tell me about the project structure."),
        ChatMessage(sender: .bobe, content: """
        The project has a clean separation between the Rust backend and the Swift frontend. \
        The backend runs as a daemon on localhost:8766 and handles all the AI logic.

        The Swift app is a transparent overlay that sits on top of your desktop. It communicates \
        with the backend via HTTP REST for commands and SSE for real-time streaming updates.

        The settings panel lets you configure everything from the AI model to the appearance. \
        Each panel is a separate SwiftUI view that loads data from the backend.

        I'd recommend starting with the overlay views if you want to understand the chat flow, \
        since that's where user interaction happens most.
        """),
    ])
    .environment(\.theme, allThemes[0])
    .frame(width: 540, height: 500)
    .background(allThemes[0].colors.background)
}
#endif
