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
    let onOpenChat: () -> Void
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

    @Environment(\.theme) private var theme
    @State private var isHovered = false

    private static let maxLines = 8
    private static let maxWidth: CGFloat = 340

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
        .transition(.asymmetric(
            insertion: .opacity.combined(with: .scale(scale: 0.96, anchor: .bottomTrailing)),
            removal: .opacity
        ))
    }

    // MARK: - Compact pill (status / silenced)

    /// Single-line capsule: `[icon] LABEL`. Label types in via
    /// `TypewriterText` for the typewriter feel. No body text — status
    /// modes are ambient indicators, the label itself is the message.
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

            TypewriterText(
                text: label,
                cursorColor: tint,
                charDelay: 0.045,
                showCursor: true
            )
            .bobeTextStyle(.chatSender)
            .tracking(0.6)
            .textCase(.uppercase)
            .foregroundStyle(tint)
            .lineLimit(1)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 5)
        .background(
            Capsule().fill(self.theme.colors.background.opacity(0.92))
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
        let clickable: Bool

        static let bobeMessage = ShellOptions(showSilence: true, showDismiss: true, clickable: true)
        static let userTranscribing = ShellOptions(showSilence: false, showDismiss: false, clickable: false)

        /// Bobe-message variant lets the caller override silence-button
        /// visibility (only shown while TTS is audible). Callsite reads
        /// stay terse with the static + with(silence:) builder pattern.
        func with(silence: Bool) -> ShellOptions {
            ShellOptions(showSilence: silence, showDismiss: self.showDismiss, clickable: self.clickable)
        }
    }

    /// Two-line bubble shell — sender row + body + optional action
    /// affordances (✕ to dismiss, silence toggle while TTS is audible).
    /// Sender label types in via `TypewriterText` for the typewriter feel.
    @ViewBuilder
    private func bubbleShell(
        sender: String,
        senderTint: Color,
        options: ShellOptions,
        @ViewBuilder bodyContent: () -> some View
    ) -> some View {
        let showSilence = options.showSilence
        let showDismiss = options.showDismiss
        let clickable = options.clickable
        // Right-padding reserves room for the action stack so text never
        // collides with the buttons.
        let actionCount = (showDismiss ? 1 : 0) + (showSilence ? 1 : 0)
        let trailingReserve: CGFloat = actionCount == 0 ? 4 : CGFloat(actionCount) * 22

        ZStack(alignment: .topTrailing) {
            let inner = VStack(alignment: .leading, spacing: 4) {
                TypewriterText(
                    text: sender,
                    cursorColor: senderTint,
                    charDelay: 0.04,
                    showCursor: false
                )
                .bobeTextStyle(.chatSender)
                .tracking(0.8)
                .textCase(.uppercase)
                .foregroundStyle(senderTint)

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
                RoundedRectangle(cornerRadius: 14).fill(self.theme.colors.background)
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

            if clickable {
                Button(action: self.onOpenChat) { inner }
                    .buttonStyle(.plain)
                    .onHover { self.isHovered = $0 }
            } else {
                inner.onHover { self.isHovered = $0 }
            }

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
                            action: self.onToggleSilence
                        )
                    }
                    if showDismiss {
                        self.iconActionButton(
                            systemName: "xmark",
                            tint: self.theme.colors.textMuted,
                            label: L10n.tr("overlay.floating_bubble.dismiss"),
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
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            Image(systemName: systemName)
                .font(.system(size: 9, weight: .semibold))
                .foregroundStyle(tint)
                .padding(5)
                .background(
                    Circle().fill(self.theme.colors.background.opacity(0.6))
                )
                .overlay(
                    Circle().strokeBorder(self.theme.colors.border.opacity(0.4), lineWidth: 0.5)
                )
        }
        .buttonStyle(.plain)
        .accessibilityLabel(label)
    }

    // MARK: - Body variants

    private func messageBody(_ message: ChatMessage) -> some View {
        // Body types in via `TypewriterText` so the user sees BoBe
        // "speaking" the words. Append mode keeps the previously-typed
        // prefix when new tokens stream in (the daemon delivers chunks
        // of growing text), instead of restarting the type-out each
        // chunk — that previously read as flashing/jittery.
        TypewriterText(
            text: message.content,
            cursorColor: self.theme.colors.primary,
            charDelay: 0.018,
            appendMode: true,
            showCursor: message.isStreaming
        )
        .bobeTextStyle(.chatBody)
        .lineLimit(Self.maxLines)
        .multilineTextAlignment(.leading)
        .fixedSize(horizontal: false, vertical: true)
    }

    private func transcribingBody(_ text: String) -> some View {
        HStack(alignment: .top, spacing: 6) {
            Image(systemName: "ear")
                .font(.system(size: 10))
                .foregroundStyle(self.theme.colors.secondary)
                .padding(.top, 2)
            Text(text)
                .bobeTextStyle(.chatBody)
                .lineLimit(Self.maxLines)
                .multilineTextAlignment(.leading)
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}
