import SwiftUI

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
                    .offset(x: 30, y: 30)

                if hasMessage && !showInput {
                    MessageBadge()
                        .offset(x: 34, y: -34)
                }
            }
            .padding(.top, 16)
            .frame(width: 116, height: 132)

            BobeLabel()
                .padding(.top, -2)
        }
        .frame(width: 132, height: 146)
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
                .frame(width: 116, height: 116)
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
                .frame(width: 76, height: 76)
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
                                endRadius: 38
                            )
                        )
                        .frame(width: 76, height: 76)
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
            .frame(width: 10, height: 10)
            .overlay(
                Circle().stroke(theme.colors.background, lineWidth: 2)
            )
            .frame(width: 14, height: 14)
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
            .frame(width: 16, height: 16)
            .overlay(
                Circle().stroke(theme.colors.background, lineWidth: 2)
            )
            .frame(width: 20, height: 20)
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
