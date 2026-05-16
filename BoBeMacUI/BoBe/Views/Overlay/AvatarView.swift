import SwiftUI

/// Geometry constants for the avatar card stack. Tightly coupled — the
/// inner-face radial gradient stops at `faceSize / 2`, the message badge
/// outer ring is twice the badge dot's diameter, etc. Adjust together.
private enum AvatarMetrics {
    /// Body of the avatar card (inner circle).
    static let cardSize: CGFloat = 116
    /// Outer frame of the avatar card including breathing room.
    static let cardFrameWidth: CGFloat = 116
    static let cardFrameHeight: CGFloat = 132
    /// Full column including the BoBe label below.
    static let columnWidth: CGFloat = 132
    static let columnHeight: CGFloat = 146
    /// Inner face circle (eyes + gradient).
    static let faceSize: CGFloat = 76
    /// Radial highlight gradient extent — half the face size.
    static let faceHighlightRadius: CGFloat = 38
    /// `ConnectionDot` offset from card center (top-right corner).
    static let connectionDotOffset: CGFloat = 30
    /// `MessageBadge` offset from card center (top-right corner, slightly
    /// further out than the connection dot so the two don't overlap).
    static let messageBadgeOffset: CGFloat = 34
    /// Connection-dot inner fill diameter and outer-ring frame.
    static let connectionDotInner: CGFloat = 10
    static let connectionDotOuter: CGFloat = 14
    /// Message-badge inner fill diameter and outer-ring frame.
    static let messageBadgeInner: CGFloat = 16
    static let messageBadgeOuter: CGFloat = 20
}

struct AvatarView: View {
    let stateType: BobeStateType
    let isCapturing: Bool
    let isConnected: Bool
    let hasMessage: Bool
    var showInput: Bool = false
    var statusOverride: String?
    var isAvatarActionEnabled = false
    var onClick: (() -> Void)?
    var onToggleCapture: (() -> Void)?

    @Environment(\.theme) private var theme
    @State private var isHovered = false
    @State private var breathingExpanded = false

    var body: some View {
        VStack(spacing: 0) {
            ZStack {
                avatarCard
                    .overlay(alignment: .top) {
                        if stateType != .speaking {
                            StatusLabel(stateType: stateType, textOverride: statusOverride)
                                .offset(y: -14)
                        }
                    }

                ConnectionDot(isConnected: isConnected)
                    .offset(x: AvatarMetrics.connectionDotOffset, y: AvatarMetrics.connectionDotOffset)

                if hasMessage && !showInput {
                    MessageBadge()
                        .offset(x: AvatarMetrics.messageBadgeOffset, y: -AvatarMetrics.messageBadgeOffset)
                }
            }
            .padding(.top, 16)
            .frame(width: AvatarMetrics.cardFrameWidth, height: AvatarMetrics.cardFrameHeight)

            BobeLabel()
                .padding(.top, -2)
        }
        .frame(width: AvatarMetrics.columnWidth, height: AvatarMetrics.columnHeight)
        .task(id: shouldBreathe) {
            breathingExpanded = false
            guard shouldBreathe else { return }
            guard OverlayMotionRuntime.shouldAnimate else { return }
            while !Task.isCancelled {
                withAnimation(OverlayMotionRuntime.animation(for: .breathing)) {
                    breathingExpanded.toggle()
                }
                try? await Task.sleep(for: .seconds(3.2))
            }
        }
    }

    private var shouldBreathe: Bool {
        switch stateType {
        case .idle, .capturing, .wantsToSpeak:
            true
        default:
            false
        }
    }

    private var motionScale: CGFloat {
        let hoverScale = OverlayMotionRuntime.hoverScale(isHovered: isHovered)
        let breathingScale = shouldBreathe ? OverlayMotionRuntime.breathingScale(isExpanded: breathingExpanded) : 1.0
        return hoverScale * breathingScale
    }

    @ViewBuilder
    private var avatarCard: some View {
        let base = ZStack {
            Circle()
                .fill(theme.colors.background)
                .frame(width: AvatarMetrics.cardSize, height: AvatarMetrics.cardSize)
                .overlay(
                    Circle().stroke(theme.colors.border, lineWidth: 2)
                )
                .shadow(color: Color.black.opacity(0.12), radius: 10, y: 4)

            if stateType == .thinking {
                ThinkingNumbersRing()
            }

            if stateType == .speaking {
                SpeakingWaveRing()
            }

            if stateType == .wantsToSpeak {
                AttentionPulse()
            }

            innerFace
        }
        .contentShape(Circle())
        .accessibilityElement(children: .combine)
        .accessibilityLabel(L10n.tr("overlay.avatar.accessibility_format", self.stateAccessibilityText))

        if self.isAvatarActionEnabled, self.onClick != nil {
            base
                .onTapGesture { self.onClick?() }
                .accessibilityAddTraits(.isButton)
        } else {
            base
        }
    }

    private var innerFace: some View {
        ZStack {
            Circle()
                .fill(
                    LinearGradient(
                        colors: [theme.colors.avatarFaceLight, theme.colors.avatarFaceDark],
                        startPoint: .init(x: 0.15, y: 0.0),
                        endPoint: .init(x: 0.85, y: 1.0)
                    )
                )
                .frame(width: AvatarMetrics.faceSize, height: AvatarMetrics.faceSize)
                .overlay(
                    Circle().stroke(theme.colors.avatarRing, lineWidth: 2)
                )
                .shadow(color: Color.black.opacity(0.15), radius: 4, y: 2)
                .overlay(
                    Circle()
                        .fill(
                            RadialGradient(
                                colors: [.white.opacity(0.25), .clear],
                                center: .init(x: 0.35, y: 0.25),
                                startRadius: 0,
                                endRadius: AvatarMetrics.faceHighlightRadius
                            )
                        )
                        .frame(width: AvatarMetrics.faceSize, height: AvatarMetrics.faceSize)
                )

            EyesIndicator(state: stateType, chatOpen: showInput)
        }
        .scaleEffect(motionScale)
        .offset(y: OverlayMotionRuntime.hoverYOffset(isHovered: isHovered))
        .onHover { hovering in
            withAnimation(OverlayMotionRuntime.animation(for: .hover)) {
                isHovered = hovering
            }
        }
        .zIndex(10)
    }

    private var stateAccessibilityText: String {
        switch self.stateType {
        case .loading: L10n.tr("overlay.avatar.state.loading")
        case .error: L10n.tr("overlay.avatar.state.error")
        case .idle: L10n.tr("overlay.avatar.state.idle")
        case .capturing: L10n.tr("overlay.avatar.state.capturing")
        case .thinking: L10n.tr("overlay.avatar.state.thinking")
        case .speaking: L10n.tr("overlay.avatar.state.speaking")
        case .wantsToSpeak: L10n.tr("overlay.avatar.state.wants_to_speak")
        case .shuttingDown: L10n.tr("overlay.avatar.state.shutting_down")
        }
    }
}

// MARK: - Connection Dot

struct ConnectionDot: View {
    let isConnected: Bool
    @Environment(\.theme) private var theme

    var body: some View {
        Circle()
            .fill(isConnected ? theme.colors.secondary : theme.colors.primary)
            .frame(width: AvatarMetrics.connectionDotInner, height: AvatarMetrics.connectionDotInner)
            .overlay(
                Circle().stroke(theme.colors.background, lineWidth: 2)
            )
            .frame(width: AvatarMetrics.connectionDotOuter, height: AvatarMetrics.connectionDotOuter)
            .accessibilityLabel(
                isConnected
                    ? L10n.tr("overlay.connection.connected")
                    : L10n.tr("overlay.connection.disconnected")
            )
    }
}

// MARK: - Message Badge

struct MessageBadge: View {
    @State private var scale: CGFloat = 1.0
    @Environment(\.theme) private var theme

    var body: some View {
        Circle()
            .fill(theme.colors.primary)
            .frame(width: AvatarMetrics.messageBadgeInner, height: AvatarMetrics.messageBadgeInner)
            .overlay(
                Circle().stroke(theme.colors.background, lineWidth: 2)
            )
            .frame(width: AvatarMetrics.messageBadgeOuter, height: AvatarMetrics.messageBadgeOuter)
            .scaleEffect(scale)
            .onAppear {
                guard OverlayMotionRuntime.shouldAnimate else {
                    scale = 1.0
                    return
                }
                withAnimation(OverlayMotionRuntime.animation(for: .badgePulse).repeatForever(autoreverses: true)) {
                    scale = 1.1
                }
            }
    }
}

// MARK: - Chat Toggle Button

struct ChatToggleButton: View {
    var isActive: Bool = false
    let action: () -> Void
    @Environment(\.theme) private var theme

    private let bubbleDiameter: CGFloat = 32

    var body: some View {
        Button(action: action) {
            ZStack {
                Circle()
                    .fill(isActive ? theme.colors.secondary : theme.colors.border)
                    .frame(width: self.bubbleDiameter, height: self.bubbleDiameter)

                Circle()
                    .stroke(theme.colors.background, lineWidth: 2)
                    .frame(width: self.bubbleDiameter, height: self.bubbleDiameter)

                Image(systemName: "message.fill")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(isActive ? theme.colors.background : theme.colors.text)
            }
        }
        .buttonStyle(.plain)
        .shadow(color: Color.black.opacity(0.1), radius: 3, y: 1)
        .frame(width: self.bubbleDiameter, height: self.bubbleDiameter)
        .contentShape(Circle())
        .accessibilityLabel(
            self.isActive
                ? L10n.tr("overlay.chat_toggle.hide.accessibility")
                : L10n.tr("overlay.chat_toggle.show.accessibility")
        )
    }
}

// MARK: - BoBe Label

struct BobeLabel: View {
    @Environment(\.theme) private var theme

    var body: some View {
        Text(L10n.tr("overlay.avatar.brand_label"))
            .bobeTextStyle(.brandLabel)
            .tracking(1.5)
            .foregroundStyle(theme.colors.primary)
            .padding(.horizontal, 7)
            .padding(.vertical, 1)
            .background(
                RoundedRectangle(cornerRadius: 6)
                    .fill(theme.colors.background)
                    .overlay(
                        RoundedRectangle(cornerRadius: 6)
                            .stroke(theme.colors.border, lineWidth: 1)
                    )
            )
            .zIndex(1)
    }
}

// MARK: - Previews

#if !SPM_BUILD
#Preview("Idle") {
    AvatarView(stateType: .idle, isCapturing: false, isConnected: true, hasMessage: false)
        .environment(\.theme, allThemes[0])
        .frame(width: 200, height: 200)
        .background(Color.gray.opacity(0.1))
}

#Preview("Thinking") {
    AvatarView(stateType: .thinking, isCapturing: false, isConnected: true, hasMessage: false)
        .environment(\.theme, allThemes[0])
        .frame(width: 200, height: 200)
        .background(Color.gray.opacity(0.1))
}

#Preview("Speaking") {
    AvatarView(stateType: .speaking, isCapturing: false, isConnected: true, hasMessage: true)
        .environment(\.theme, allThemes[0])
        .frame(width: 200, height: 200)
        .background(Color.gray.opacity(0.1))
}

#Preview("Error + Message") {
    AvatarView(stateType: .error, isCapturing: false, isConnected: false, hasMessage: true)
        .environment(\.theme, allThemes[0])
        .frame(width: 200, height: 200)
        .background(Color.gray.opacity(0.1))
}

#Preview("Wants to Speak") {
    AvatarView(stateType: .wantsToSpeak, isCapturing: false, isConnected: true, hasMessage: false)
        .environment(\.theme, allThemes[0])
        .frame(width: 200, height: 200)
        .background(Color.gray.opacity(0.1))
}
#endif
