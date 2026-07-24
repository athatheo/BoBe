import SwiftUI

/// The single ambient surface above the avatar. Replaces the previous
/// constellation of separate components (FloatingBobeBubble, the
/// status pill above the avatar head, the SilencedPill, the inline
/// daemon-error banner-below-the-avatar) with one shape that morphs
/// content based on what BoBe is doing right now.
///
/// Two visual treatments:
/// - **Compact pill** (single line, capsule) for status + silenced.
///   These are ambient "what BoBe is doing" indicators — they should
///   feel lightweight and not steal focus from the avatar.
/// - **Full bubble** (two-line shell) for BoBe messages + live STT.
///   These carry actual content the user wants to read.
///
/// Daemon-disconnect errors are intentionally NOT shown here — the
/// `errorBannerSection` handles them with an actionable Restart button.
/// Two surfaces for the same error read as broken.
///
/// Mode priority is enforced by the caller (`OverlayView.bubbleMode`);
/// this view just renders what it's told.
struct AvatarStatusBubble: View {
    enum Mode {
        case hidden
        case bobeMessage(ChatMessage)
        case userTranscribing(text: String)
        case status(kind: StatusKind, body: String?)
        case silenced

        /// Activity that BoBe is performing — drives icon + default body
        /// copy when no daemon-supplied label is available.
        enum StatusKind: Equatable {
            case thinking
            case capturing
            case toolRunning(name: String)
            case reconnecting
            case starting

            var icon: String {
                switch self {
                case .thinking: "brain"
                case .capturing: "camera.viewfinder"
                case .toolRunning: "wrench.fill"
                case .reconnecting: "arrow.triangle.2.circlepath"
                case .starting: "hourglass"
                }
            }

            var label: String {
                switch self {
                case .thinking: L10n.tr("overlay.status.thinking")
                case .capturing: L10n.tr("overlay.status.capturing")
                case let .toolRunning(name): L10n.tr("overlay.status.using_tool_format", name)
                case .reconnecting: L10n.tr("overlay.reconnecting")
                case .starting: L10n.tr("overlay.status.starting")
                }
            }
        }
    }

    let mode: Mode
    let onOpenAnswer: () -> Void
    let onToggleSilence: () -> Void
    /// Dismiss the currently-displayed BoBe message bubble. Only wired
    /// for the `.bobeMessage` mode — status pills and live STT auto-clear
    /// when their underlying signal goes away.
    let onDismiss: () -> Void
    let isSilenced: Bool
    /// True when TTS audio is actually playing right now (not just text
    /// streaming). The silence button only appears in that window — there's
    /// nothing meaningful to silence when BoBe isn't talking.
    let isTtsAudible: Bool
    let dismissalStyle: AmbientBubbleDismissalStyle
    let recedeProgress: Double
    let onInteractionChanged: (Bool) -> Void

    @Environment(\.theme) private var theme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var isHovered = false
    @State private var messageContentHeight: CGFloat = 20
    @State private var messageScrolledToBottom = false
    @FocusState private var focusedAction: BubbleAction?
    @AccessibilityFocusState private var accessibilityFocus: BubbleAccessibilityTarget?

    private static let maxWidth: CGFloat = 340
    private static let transcriptBottomAnchor = "ambient-transcript-bottom"

    private enum BubbleAction: Hashable {
        case openAnswer
        case silence
        case dismiss
    }

    private enum BubbleAccessibilityTarget: Hashable {
        case sender
        case message
        case openAnswer
        case silence
        case dismiss
    }

    var body: some View {
        Group {
            switch self.mode {
            case .hidden:
                EmptyView()
            case let .bobeMessage(message):
                self.bubbleShell(
                    sender: L10n.tr("overlay.chat.sender.bobe"),
                    senderTint: self.theme.colors.primary,
                    options: .bobeMessage.with(silence: self.isTtsAudible),
                    bodyContent: { self.messageBody(message) }
                )
            case let .userTranscribing(text):
                self.bubbleShell(
                    sender: L10n.tr("overlay.bubble.sender.you"),
                    senderTint: self.theme.colors.secondary,
                    options: .userTranscribing,
                    bodyContent: { self.transcribingBody(text) }
                )
            case let .status(kind, _):
                self.compactPill(
                    icon: kind.icon,
                    label: kind.label,
                    tint: self.theme.colors.primary,
                    onTap: nil
                )
            case .silenced:
                self.compactPill(
                    icon: "speaker.slash.fill",
                    label: L10n.tr("overlay.bubble.sender.silenced"),
                    tint: self.theme.colors.textMuted,
                    onTap: self.onToggleSilence
                )
            }
        }
        .frame(maxWidth: Self.maxWidth, alignment: .trailing)
        .modifier(
            AmbientBubbleTransition(
                style: self.reduceMotion ? .fade : self.dismissalStyle,
                progress: self.recedeProgress
            )
        )
        .transition(.opacity)
        .onChange(of: self.focusedAction) { _, _ in
            self.reportInteraction()
        }
        .onChange(of: self.accessibilityFocus) { _, _ in
            self.reportInteraction()
        }
    }

    // MARK: - Compact pill (status / silenced)

    /// Single-line capsule: `[icon] LABEL`. No body text — status modes
    /// are ambient indicators, the label itself is the message.
    /// `onTap` makes the whole pill clickable (used by silenced mode
    /// to toggle silence off in one tap).
    @ViewBuilder
    private func compactPill(
        icon: String,
        label: String,
        tint: Color,
        onTap: (() -> Void)?
    ) -> some View {
        let content = HStack(spacing: 6) {
            Image(systemName: icon)
                .font(.system(size: 10, weight: .semibold))
                .foregroundStyle(tint)

            Text(label)
            .bobeTextStyle(.chatSender)
            .tracking(0.6)
            .textCase(.uppercase)
            .foregroundStyle(tint)
            .lineLimit(1)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 5)
        .background(
            Capsule().fill(self.theme.colors.surface)
        )
        .overlay(
            Capsule().strokeBorder(self.theme.colors.border.opacity(0.6), lineWidth: 0.5)
        )
        .shadow(color: self.theme.colors.text.opacity(0.08), radius: 4, y: 2)
        .fixedSize(horizontal: true, vertical: true)

        if let onTap {
            Button(action: onTap) { content }
                .buttonStyle(.plain)
        } else {
            content
        }
    }

    // MARK: - Full bubble (message / transcribing)

    /// Knobs the bubble shell exposes to its two call sites (BoBe message
    /// vs user-transcribing). Grouped into a value type so the shell's
    /// signature stays under SwiftLint's param-count cap and so future
    /// affordances can land here without churning every callsite.
    struct ShellOptions {
        let showSilence: Bool
        let showDismiss: Bool

        static let bobeMessage = ShellOptions(showSilence: true, showDismiss: true)
        static let userTranscribing = ShellOptions(showSilence: false, showDismiss: false)

        /// Bobe-message variant lets the caller override silence-button
        /// visibility (only shown while TTS is audible). Callsite reads
        /// stay terse with the static + with(silence:) builder pattern.
        func with(silence: Bool) -> ShellOptions {
            ShellOptions(showSilence: silence, showDismiss: self.showDismiss)
        }
    }

    /// Message bubble shell — sender row + body + optional action
    /// affordances (dismiss and silence while TTS is audible).
    @ViewBuilder
    private func bubbleShell(
        sender: String,
        senderTint: Color,
        options: ShellOptions,
        @ViewBuilder bodyContent: () -> some View
    ) -> some View {
        let showSilence = options.showSilence
        let showDismiss = options.showDismiss
        // Right-padding reserves room for the action stack so text never
        // collides with the buttons.
        let actionCount = (showDismiss ? 1 : 0) + (showSilence ? 1 : 0)
        let trailingReserve: CGFloat = actionCount == 0 ? 4 : CGFloat(actionCount) * 22

        ZStack(alignment: .topTrailing) {
            let inner = VStack(alignment: .leading, spacing: 4) {
                Text(sender)
                .bobeTextStyle(.chatSender)
                .tracking(0.8)
                .textCase(.uppercase)
                .foregroundStyle(senderTint)
                .accessibilityFocused(self.$accessibilityFocus, equals: .sender)

                bodyContent()
                    .foregroundStyle(self.theme.colors.text)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.trailing, trailingReserve)
            .padding(.horizontal, 10)
            .padding(.vertical, 9)
            .frame(maxWidth: Self.maxWidth, alignment: .leading)
            .background(
                RoundedRectangle(cornerRadius: 14).fill(self.theme.colors.surface)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 14)
                    .strokeBorder(
                        self.isHovered
                            ? self.theme.colors.primary.opacity(0.55)
                            : self.theme.colors.border,
                        lineWidth: 1
                    )
            )
            .shadow(color: self.theme.colors.text.opacity(0.10), radius: 6, y: 3)
            .contentShape(RoundedRectangle(cornerRadius: 14))
            .onHover { hovering in
                self.isHovered = hovering
                self.reportInteraction()
            }
            inner

            if actionCount > 0 {
                HStack(spacing: 4) {
                    if showSilence {
                        self.iconActionButton(
                            systemName: self.isSilenced
                                ? "speaker.slash.fill"
                                : "speaker.wave.2.fill",
                            tint: self.isSilenced
                                ? self.theme.colors.primary
                                : self.theme.colors.textMuted,
                            label: L10n.tr(
                                self.isSilenced
                                    ? "overlay.floating_bubble.unsilence"
                                    : "overlay.floating_bubble.silence"
                            ),
                            focus: .silence,
                            accessibilityFocus: .silence,
                            action: self.onToggleSilence
                        )
                    }
                    if showDismiss {
                        self.iconActionButton(
                            systemName: "xmark",
                            tint: self.theme.colors.textMuted,
                            label: L10n.tr("overlay.floating_bubble.dismiss"),
                            focus: .dismiss,
                            accessibilityFocus: .dismiss,
                            action: self.onDismiss
                        )
                    }
                }
                .padding(6)
            }
        }
    }

    /// Small circular icon button used by the bubble's action row. One
    /// implementation for both silence and dismiss so they stay visually
    /// consistent (same size, same hover treatment, same chrome).
    private func iconActionButton(
        systemName: String,
        tint: Color,
        label: String,
        focus: BubbleAction,
        accessibilityFocus: BubbleAccessibilityTarget,
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            Image(systemName: systemName)
                .font(.system(size: 9, weight: .semibold))
                .foregroundStyle(tint)
                .padding(5)
                .background(
                    Circle().fill(self.theme.colors.background)
                )
                .overlay(
                    Circle().strokeBorder(self.theme.colors.border.opacity(0.4), lineWidth: 0.5)
                )
        }
        .buttonStyle(.plain)
        .focused(self.$focusedAction, equals: focus)
        .accessibilityFocused(self.$accessibilityFocus, equals: accessibilityFocus)
        .accessibilityLabel(label)
    }

    // MARK: - Body variants

    private func messageBody(_ message: ChatMessage) -> some View {
        let shouldScroll = self.messageContentHeight > WindowSizes.heightAmbientMessageViewport + 1
        let overflowAffordanceHeight: CGFloat = shouldScroll ? 32 : 0
        let viewportHeight = min(
            max(20, self.messageContentHeight),
            WindowSizes.heightAmbientMessageViewport - overflowAffordanceHeight
        )
        let fadeStart = max(0, 1 - (14 / max(viewportHeight, 1)))
        return VStack(alignment: .leading, spacing: 2) {
            ScrollView(.vertical, showsIndicators: shouldScroll) {
                VStack(alignment: .leading, spacing: 6) {
                    HStack(alignment: .bottom, spacing: 1) {
                        Text(message.content)
                            .bobeTextStyle(.chatBody)
                            .multilineTextAlignment(.leading)
                            .fixedSize(horizontal: false, vertical: true)

                        if message.isStreaming {
                            StreamingCursor(color: self.theme.colors.primary)
                        }
                    }
                    if !message.isComplete {
                        Label(
                            L10n.tr("overlay.chat.interrupted"),
                            systemImage: "exclamationmark.circle"
                        )
                        .bobeTextStyle(.chatPending)
                        .foregroundStyle(self.theme.colors.error)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .background {
                    GeometryReader { geometry in
                        Color.clear.preference(
                            key: AmbientMessageContentHeightKey.self,
                            value: ceil(geometry.size.height)
                        )
                    }
                }
            }
            .frame(height: viewportHeight)
            .scrollDisabled(!shouldScroll)
            .scrollBounceBehavior(.basedOnSize)
            .textSelection(.enabled)
            .onScrollGeometryChange(for: Bool.self) { geometry in
                geometry.visibleRect.maxY >= geometry.contentSize.height - 1
            } action: { _, isAtBottom in
                self.messageScrolledToBottom = isAtBottom
            }
            .mask {
                if shouldScroll, !self.messageScrolledToBottom {
                    LinearGradient(
                        stops: [
                            .init(color: .black, location: 0),
                            .init(color: .black, location: fadeStart),
                            .init(color: .clear, location: 1),
                        ],
                        startPoint: .top,
                        endPoint: .bottom
                    )
                } else {
                    Rectangle().fill(.black)
                }
            }
            .accessibilityElement(children: .ignore)
            .accessibilityLabel(message.content)
            .accessibilityValue(
                message.isStreaming
                    ? L10n.tr("overlay.status.thinking")
                    : message.isComplete
                        ? ""
                        : L10n.tr("overlay.chat.interrupted")
            )
            .accessibilityFocused(self.$accessibilityFocus, equals: .message)

            if shouldScroll {
                Button(action: self.onOpenAnswer) {
                    Label(
                        L10n.tr("overlay.floating_bubble.open_full_answer"),
                        systemImage: "arrow.up.left.and.arrow.down.right"
                    )
                    .bobeTextStyle(.chatPending)
                    .foregroundStyle(self.theme.colors.primary)
                    .frame(maxWidth: .infinity, alignment: .trailing)
                }
                .buttonStyle(.plain)
                .focused(self.$focusedAction, equals: .openAnswer)
                .accessibilityFocused(self.$accessibilityFocus, equals: .openAnswer)
                .frame(maxWidth: .infinity, minHeight: 22, alignment: .trailing)
                .contentShape(Rectangle())
                .accessibilityIdentifier("overlay.bubble.open-full-answer")
            }
        }
        .onPreferenceChange(AmbientMessageContentHeightKey.self) { height in
            if height > 0 {
                self.messageContentHeight = height
            }
        }
        .onChange(of: message.id) { _, _ in
            self.messageContentHeight = 20
            self.messageScrolledToBottom = false
        }
    }

    private func transcribingBody(_ text: String) -> some View {
        ScrollViewReader { proxy in
            ScrollView(.vertical, showsIndicators: true) {
                VStack(alignment: .leading, spacing: 0) {
                    HStack(alignment: .top, spacing: 6) {
                        Image(systemName: "ear")
                            .font(.system(size: 10))
                            .foregroundStyle(self.theme.colors.secondary)
                            .padding(.top, 2)
                        Text(text)
                            .bobeTextStyle(.chatBody)
                            .multilineTextAlignment(.leading)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)

                    Color.clear
                        .frame(height: 1)
                        .id(Self.transcriptBottomAnchor)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .frame(maxHeight: WindowSizes.heightAmbientMessageViewport)
            .scrollBounceBehavior(.basedOnSize)
            .defaultScrollAnchor(.bottom)
            .onAppear {
                proxy.scrollTo(Self.transcriptBottomAnchor, anchor: .bottom)
            }
            .onChange(of: text) { _, _ in
                proxy.scrollTo(Self.transcriptBottomAnchor, anchor: .bottom)
            }
        }
    }

    private func reportInteraction() {
        self.onInteractionChanged(
            self.isHovered || self.focusedAction != nil || self.accessibilityFocus != nil
        )
    }
}

private struct AmbientMessageContentHeightKey: PreferenceKey {
    static let defaultValue: CGFloat = 0

    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) {
        value = max(value, nextValue())
    }
}
