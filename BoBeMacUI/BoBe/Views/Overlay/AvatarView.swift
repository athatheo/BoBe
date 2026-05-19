import SwiftUI

/// Geometry constants for the avatar card stack. Tightly coupled — the
/// inner-face radial gradient stops at `faceSize / 2`, the message badge
/// outer ring is twice the badge dot's diameter, etc. Adjust together.
///
/// `internal` so callers that need to pin satellites to the rim
/// (OverlaySections.avatarCluster) compute positions from these same
/// numbers — when constants change here, the satellites stay aligned.
enum AvatarMetrics {
    /// Body of the avatar card (inner circle); also the frame width.
    static let cardSize: CGFloat = 116
    /// Card frame height = `cardSize` + vertical breathing room (16) so the
    /// avatar isn't flush against the BobeLabel below.
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
    /// Voice-presence ring base diameter — sits just outside the avatar card
    /// stroke. Driven by `VoicePipeline.inputLevel` so the user sees their
    /// voice modulating the ring while BoBe listens.
    static let voicePresenceBaseDiameter: CGFloat = 126
    /// Max additional diameter at peak inputLevel (1.0).
    static let voicePresenceExpansionRange: CGFloat = 18

    /// Top-leading offset for placing a satellite circle on the avatar
    /// card's rim. Designed for use inside
    /// `.overlay(alignment: .topLeading)` on the avatar card itself —
    /// the only inputs are `cardSize` (the rim) and `satelliteSize` (the
    /// child). No dependency on column padding, BobeLabel positioning,
    /// or any other outer-layout decision, so changes to those won't
    /// silently break alignment.
    ///
    /// `angleFrom12` is degrees clockwise from 12 o'clock:
    ///   0=top, 90=right, 180=bottom, 270=left, 315=10:30, 225=7:30.
    ///
    /// `rimOverlap` controls how much of the satellite eats into the
    /// card. `0` = satellite's inner edge tangent to the outer rim
    /// (fully outside the card). `satelliteSize/2` = satellite centre
    /// ON the rim line (half inside, half outside — pre-fix behaviour
    /// that visually buried the satellite into the inner face). The
    /// default (`8`) is a small intentional overlap so the satellite
    /// reads as anchored to the card rather than floating beside it.
    static func rimOffset(
        angleFrom12: Double,
        satelliteSize: CGFloat,
        rimOverlap: CGFloat = 8
    ) -> CGSize {
        let cardR = self.cardSize / 2
        let satR = satelliteSize / 2
        let theta = angleFrom12 * .pi / 180
        // Distance from card centre to satellite centre. Larger than
        // cardR pushes the satellite OUTSIDE the rim; `rimOverlap`
        // pulls it back in by that many points.
        let centreDistance = cardR + satR - rimOverlap
        // Card centre in overlay coords is (cardR, cardR).
        return CGSize(
            width: cardR + centreDistance * sin(theta) - satR,
            height: cardR - centreDistance * cos(theta) - satR
        )
    }
}

/// AvatarView — purely visual.
///
/// Renders BoBe's face, the per-state rings (thinking / speaking /
/// wantsToSpeak), the breathing animation, the connection dot, the
/// message badge, the status pill above the head, and the BoBe label
/// below. **No interaction** — clicks pass through. The chat-toggle and
/// mic buttons that visually sit on the rim are now composed as
/// peers in `OverlaySections.avatarCluster`, not inside this view.
///
/// History: AvatarView used to accept a `cardSatellites: @ViewBuilder`
/// closure (generic on a `Satellites: View` type parameter) AND an
/// `onClick`/`isAvatarActionEnabled` pair that wrapped the entire card
/// in a Button. Two issues with that design:
///   1. The Button's `.contentShape(Circle())` constrained hit-testing
///      to the inner circle, but the satellites — positioned at the rim
///      via `AvatarMetrics.rimOffset` with an 8pt overlap — sat OUTSIDE
///      that circle, so they were visible but unclickable.
///   2. AvatarView mixed visual + interactive concerns. Three callers
///      filled the satellite hole; one filled the click action.
/// Both go away once the avatar body click is dropped. The chat lives
/// behind its own affordance (the chat-toggle satellite, plus floating
/// bubble taps); we don't need the avatar body as an additional target.
struct AvatarView: View {
    let stateType: BobeStateType
    let isConnected: Bool
    let hasMessage: Bool
    var showInput: Bool = false
    var statusOverride: String?
    /// When `true`, the small typewriter pill above the avatar head shows
    /// the current status ("Thinking...", "Capturing", etc.). When
    /// `false`, the label is suppressed because some other surface — the
    /// AvatarStatusBubble — is carrying the status duty instead. Avoids
    /// having two "thinking" indicators stacked on top of each other when
    /// the chat is collapsed and the bubble is active.
    var showStatusLabel: Bool = true
    /// True when the AvatarStatusBubble above the avatar is currently
    /// showing the latest BoBe message. In that case the top-right
    /// MessageBadge pulse becomes redundant (the bubble already carries
    /// the unread signal AND the actual content), so we suppress it to
    /// avoid two indicators saying the same thing.
    var bubbleShowingMessage: Bool = false

    @Environment(\.theme) private var theme
    @State private var isHovered = false
    @State private var breathingExpanded = false
}

extension AvatarView {
    var body: some View {
        VStack(spacing: 0) {
            ZStack {
                self.avatarCard
                    .overlay(alignment: .top) {
                        if self.showStatusLabel, self.stateType != .speaking {
                            StatusLabel(stateType: self.stateType, textOverride: self.statusOverride)
                                .offset(y: -14)
                        }
                    }

                ConnectionDot(isConnected: self.isConnected)
                    .offset(x: AvatarMetrics.connectionDotOffset, y: AvatarMetrics.connectionDotOffset)

                if self.hasMessage, !self.showInput, !self.bubbleShowingMessage {
                    MessageBadge()
                        .offset(x: AvatarMetrics.messageBadgeOffset, y: -AvatarMetrics.messageBadgeOffset)
                }
            }
            .padding(.top, self.showStatusLabel ? 16 : 0)
            .frame(width: AvatarMetrics.cardSize, height: AvatarMetrics.cardFrameHeight)

            BobeLabel()
                .padding(.top, -11)
        }
        .frame(width: AvatarMetrics.columnWidth, height: AvatarMetrics.columnHeight)
        .task(id: self.shouldBreathe) {
            // Reset MUST be in a no-animation transaction. Without this,
            // any ambient `withAnimation` in a parent (`OverlaySections`
            // stacks three on the avatar section) can capture the
            // `breathingExpanded = false` write and tween it across the
            // shouldBreathe transition — which read as the avatar
            // snapping mid-state-change.
            withTransaction(Transaction(animation: nil)) {
                self.breathingExpanded = false
            }
            guard self.shouldBreathe, OverlayMotionRuntime.shouldAnimate else { return }
            while !Task.isCancelled {
                withAnimation(OverlayMotionRuntime.animation(for: .breathing)) {
                    self.breathingExpanded.toggle()
                }
                try? await Task.sleep(for: .seconds(3.2))
            }
        }
    }

    private var shouldBreathe: Bool {
        switch self.stateType {
        case .idle, .capturing, .wantsToSpeak:
            true
        default:
            false
        }
    }

    private var motionScale: CGFloat {
        let hoverScale = OverlayMotionRuntime.hoverScale(isHovered: self.isHovered)
        let breathingScale = self.shouldBreathe ? OverlayMotionRuntime.breathingScale(isExpanded: self.breathingExpanded) : 1.0
        return hoverScale * breathingScale
    }

    /// The avatar card itself — voice-presence ring, the circular body,
    /// the per-state ring overlay, and the inner face. No interactivity,
    /// no hit shape: clicks pass straight through to whatever is layered
    /// above (the chat-toggle and mic satellites in `OverlaySections.
    /// avatarCluster`). The accessibility element bundles the children
    /// so VoiceOver reads "BoBe is thinking" / etc. as one node.
    private var avatarCard: some View {
        ZStack {
            // Voice-presence ring sits BEHIND the card so the card's solid
            // fill + border still reads as the avatar surface. Driven by
            // `VoicePipeline.inputLevel`; rendered only when mic is open.
            AvatarVoicePresenceRing()

            Circle()
                .fill(self.theme.colors.background)
                .frame(width: AvatarMetrics.cardSize, height: AvatarMetrics.cardSize)
                .overlay(
                    Circle().stroke(self.theme.colors.border, lineWidth: 2)
                )
                .shadow(color: self.theme.colors.text.opacity(0.12), radius: 10, y: 4)

            if self.stateType == .thinking {
                ThinkingNumbersRing()
            }

            if self.stateType == .speaking {
                SpeakingWaveRing()
            }

            if self.stateType == .wantsToSpeak {
                AttentionPulse()
            }

            self.innerFace
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel(L10n.tr("overlay.avatar.accessibility_format", self.stateAccessibilityText))
    }

    private var innerFace: some View {
        ZStack {
            Circle()
                .fill(
                    LinearGradient(
                        colors: [self.theme.colors.avatarFaceLight, self.theme.colors.avatarFaceDark],
                        startPoint: .init(x: 0.15, y: 0.0),
                        endPoint: .init(x: 0.85, y: 1.0)
                    )
                )
                .frame(width: AvatarMetrics.faceSize, height: AvatarMetrics.faceSize)
                .overlay(
                    Circle().stroke(self.theme.colors.avatarRing, lineWidth: 2)
                )
                .shadow(color: self.theme.colors.text.opacity(0.14), radius: 4, y: 2)
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

            EyesIndicator(state: self.stateType, chatOpen: self.showInput)
        }
        .scaleEffect(self.motionScale)
        .offset(y: OverlayMotionRuntime.hoverYOffset(isHovered: self.isHovered))
        .onHover { hovering in
            withAnimation(OverlayMotionRuntime.animation(for: .hover)) {
                self.isHovered = hovering
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

// MARK: - Voice Presence Ring

/// Turbulent, voice-modulated ring around the avatar. Replaces the
/// mic-button's pulsing input ring so BoBe itself reads as the listener;
/// the mic becomes a quiet on/off toggle.
///
/// The ring is NOT a perfect circle. Its radius is a parametric function
/// of angle θ and time t:
///
///   r(θ, t) = R + low(θ, t) + mid(θ, t, level) + high(θ, t, level²)
///
/// where each band is `aᵢ · sin(fᵢ·θ + ωᵢ·t + φᵢ)`. Three bands give the
/// blob personality:
///   • low  (f=2, slow ω) — always-on breathing wobble, alive at silence
///   • mid  (f=5, medium ω, amplitude ∝ level) — the user's voice
///   • high (f=11, fast ω, amplitude ∝ level²) — turbulent edge under speech
///
/// Transparency-safe: rendered as two stacked strokes (tinted halo + brand
/// primary) so the silhouette holds against any desktop wallpaper.
/// On reduce-motion we render a still, slightly noisy outline at midpoint
/// level so the affordance is still legible without animation.
struct AvatarVoicePresenceRing: View {
    private let pipeline = VoicePipeline.shared
    @State private var wakeStarted: Date?
    @State private var hasShownWake = false
    @Environment(\.theme) private var theme

    /// Visible only when the daemon could plausibly be hearing audio. While
    /// BoBe thinks / speaks / is idle the avatar uses its own state rings
    /// (`ThinkingNumbersRing`, `SpeakingWaveRing`) — stacking another ring
    /// on top would be noise.
    private var isVisible: Bool {
        switch self.pipeline.state {
        case .listening, .capturing: true
        default: false
        }
    }

    var body: some View {
        Group {
            if self.isVisible {
                if OverlayMotionRuntime.shouldAnimate {
                    self.animatedRing
                } else {
                    self.staticRing
                }
            }
        }
        .onChange(of: self.isVisible) { _, newValue in
            if newValue, !self.hasShownWake, OverlayMotionRuntime.shouldAnimate {
                self.hasShownWake = true
                self.wakeStarted = .now
            }
        }
        // Make the ring meaningful to screen readers and to anyone who
        // hovers (NSPanel tooltip). The ring is silent decoration if you
        // don't tell users "this is BoBe hearing you" somewhere.
        .accessibilityElement()
        .accessibilityLabel(L10n.tr("overlay.voice.presence_ring.accessibility"))
        .help(L10n.tr("overlay.voice.presence_ring.tooltip"))
    }

    /// Live render: `TimelineView(.animation)` drives the phase off the
    /// system clock so the wobble is smooth at whatever refresh rate the
    /// display delivers (60 / 120 / ProMotion). We deliberately read
    /// `pipeline.inputLevel` each tick rather than animating to it — the
    /// pipeline already smooths the RMS, and a SwiftUI `withAnimation`
    /// layer would just add lag.
    private var animatedRing: some View {
        TimelineView(.animation(minimumInterval: 1.0 / 60.0, paused: false)) { context in
            let t = context.date.timeIntervalSinceReferenceDate
            let level = CGFloat(self.pipeline.inputLevel)
            let wakeBoost = self.wakeBoost(now: context.date)
            // Resting state still breathes via the low-frequency band, so we
            // floor `level` at a small idle value to keep the mid-band
            // contributing a hint of motion even in silence.
            let effectiveLevel = max(level, 0.08)
            self.ringStack(phase: t, level: effectiveLevel, wakeBoost: wakeBoost)
        }
    }

    private var staticRing: some View {
        // Reduce-motion: hold at midpoint with a small phase so the blob is
        // visibly non-circular but doesn't move.
        self.ringStack(phase: 1.5, level: 0.3, wakeBoost: 0)
    }

    /// Eases the wake "expand and settle" pulse over ~900ms after the
    /// first listen of the session. After that, returns 0.
    private func wakeBoost(now: Date) -> CGFloat {
        guard let started = self.wakeStarted else { return 0 }
        let elapsed = now.timeIntervalSince(started)
        guard elapsed < 0.9 else { return 0 }
        // 0 → 1 (out-quart) over 0..0.35s, then 1 → 0 (out-quint) over 0.35..0.9s.
        if elapsed < 0.35 {
            let p = elapsed / 0.35
            return CGFloat(1 - pow(1 - p, 4)) * 14
        } else {
            let p = (elapsed - 0.35) / 0.55
            return CGFloat(1 - pow(p, 5)) * 14
        }
    }

    @ViewBuilder
    private func ringStack(phase: Double, level: CGFloat, wakeBoost: CGFloat) -> some View {
        let diameter = AvatarMetrics.voicePresenceBaseDiameter
            + level * AvatarMetrics.voicePresenceExpansionRange
            + wakeBoost
        ZStack {
            TurbulentRingShape(phase: phase, level: level)
                .stroke(self.theme.colors.text.opacity(0.22), lineWidth: 4)
                .frame(width: diameter, height: diameter)
            TurbulentRingShape(phase: phase, level: level)
                .stroke(
                    self.theme.colors.primary.opacity(0.5 + Double(level) * 0.5),
                    lineWidth: 2.5
                )
                .frame(width: diameter, height: diameter)
        }
        .transition(.opacity)
    }
}

/// Per-vertex angular sample for `TurbulentRingShape`. Holding `theta`
/// plus its pre-computed cos / sin saves ~194 trig calls per frame (the
/// ring renders at 60+ Hz with two stacked strokes). A named struct here
/// reads more clearly than a 3-tuple and dodges SwiftLint's tuple-arity
/// warning at no runtime cost.
private struct RingThetaSample {
    let theta: Double
    let cos: Double
    let sin: Double
}

private let ringSegmentCount = 96
private let ringThetaTable: [RingThetaSample] = {
    var entries: [RingThetaSample] = []
    entries.reserveCapacity(ringSegmentCount + 1)
    for i in 0 ... ringSegmentCount {
        let theta = Double(i) / Double(ringSegmentCount) * 2 * .pi
        entries.append(RingThetaSample(theta: theta, cos: cos(theta), sin: sin(theta)))
    }
    return entries
}()

/// Parametric blob whose radius is a sum of three sine bands. Drawn as a
/// closed `Path` with 96 vertices around the circumference; that's dense
/// enough that the high-frequency band (f=11) doesn't alias visibly, and
/// cheap enough to render at 120 Hz without breaking a sweat on M-series.
struct TurbulentRingShape: Shape {
    var phase: Double
    var level: CGFloat

    /// Animate phase + level smoothly when the shape is wrapped in an
    /// implicit animation. `TimelineView` updates `phase` directly, so this
    /// mostly matters for the level-driven amplitude changes.
    var animatableData: AnimatablePair<Double, CGFloat> {
        get { AnimatablePair(self.phase, self.level) }
        set {
            self.phase = newValue.first
            self.level = newValue.second
        }
    }

    func path(in rect: CGRect) -> Path {
        var path = Path()
        let center = CGPoint(x: rect.midX, y: rect.midY)
        // Reserve space for the wobble amplitude so the shape stays inside
        // its frame even at peak displacement.
        let baseRadius = Double(min(rect.width, rect.height) / 2) - 4
        let levelD = Double(self.level)

        // Per-band amplitudes. Tuned by eye:
        //   • low: always-present breathing; 1.4pt amplitude.
        //   • mid: scales linearly with voice level up to ~2.6pt.
        //   • high: scales quadratically with level for the turbulent edge
        //     under loud speech, capped at ~3.6pt to avoid spikes.
        let lowAmp = 1.4
        let midAmp = 2.6 * levelD
        let highAmp = 3.6 * (levelD * levelD)

        // Per-band angular frequencies (number of bumps around the ring)
        // and time frequencies (how fast each band rotates its phase).
        let lowAng = 2.0
        let midAng = 5.0
        let highAng = 11.0
        let lowOmega = 0.5
        let midOmega = 1.3
        let highOmega = 2.4

        // Static phase offsets break the three bands out of lockstep so they
        // never align into a clean symmetric petal pattern.
        let lowPhi = 0.0
        let midPhi = 1.7
        let highPhi = 3.1

        for (i, entry) in ringThetaTable.enumerated() {
            let low = lowAmp * sin(lowAng * entry.theta + lowOmega * self.phase + lowPhi)
            let mid = midAmp * sin(midAng * entry.theta + midOmega * self.phase + midPhi)
            let high = highAmp * sin(highAng * entry.theta + highOmega * self.phase + highPhi)
            let radius = baseRadius + low + mid + high
            let point = CGPoint(
                x: center.x + CGFloat(radius * entry.cos),
                y: center.y + CGFloat(radius * entry.sin)
            )
            if i == 0 {
                path.move(to: point)
            } else {
                path.addLine(to: point)
            }
        }
        path.closeSubpath()
        return path
    }
}

// MARK: - Connection Dot

struct ConnectionDot: View {
    let isConnected: Bool
    @Environment(\.theme) private var theme

    var body: some View {
        Circle()
            .fill(self.isConnected ? self.theme.colors.secondary : self.theme.colors.primary)
            .frame(width: AvatarMetrics.connectionDotInner, height: AvatarMetrics.connectionDotInner)
            .overlay(
                Circle().stroke(self.theme.colors.background, lineWidth: 2)
            )
            .frame(width: AvatarMetrics.connectionDotOuter, height: AvatarMetrics.connectionDotOuter)
            .accessibilityLabel(
                self.isConnected
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
            .fill(self.theme.colors.primary)
            .frame(width: AvatarMetrics.messageBadgeInner, height: AvatarMetrics.messageBadgeInner)
            .overlay(
                Circle().stroke(self.theme.colors.background, lineWidth: 2)
            )
            .frame(width: AvatarMetrics.messageBadgeOuter, height: AvatarMetrics.messageBadgeOuter)
            .scaleEffect(self.scale)
            .task {
                guard OverlayMotionRuntime.shouldAnimate else {
                    self.scale = 1.0
                    return
                }
                withAnimation(OverlayMotionRuntime.animation(for: .badgePulse).repeatForever(autoreverses: true)) {
                    self.scale = 1.1
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
        Button(action: self.action) {
            ZStack {
                Circle()
                    .fill(self.isActive ? self.theme.colors.secondary : self.theme.colors.border)
                    .frame(width: self.bubbleDiameter, height: self.bubbleDiameter)

                Circle()
                    .stroke(self.theme.colors.background, lineWidth: 2)
                    .frame(width: self.bubbleDiameter, height: self.bubbleDiameter)

                Image(systemName: "message.fill")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(self.isActive ? self.theme.colors.background : self.theme.colors.text)
            }
        }
        .buttonStyle(.plain)
        .shadow(color: self.theme.colors.text.opacity(0.10), radius: 3, y: 1)
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
            .foregroundStyle(self.theme.colors.primary)
            .padding(.horizontal, 7)
            .padding(.vertical, 1)
            .background(
                RoundedRectangle(cornerRadius: 6)
                    .fill(self.theme.colors.background)
                    .overlay(
                        RoundedRectangle(cornerRadius: 6)
                            .stroke(self.theme.colors.border, lineWidth: 1)
                    )
            )
            .zIndex(1)
    }
}

// MARK: - Previews

#if !SPM_BUILD
    #Preview("Idle") {
        AvatarView(stateType: .idle, isConnected: true, hasMessage: false)
            .environment(\.theme, allThemes[0])
            .frame(width: 200, height: 200)
            .background(Color.gray.opacity(0.1))
    }

    #Preview("Thinking") {
        AvatarView(stateType: .thinking, isConnected: true, hasMessage: false)
            .environment(\.theme, allThemes[0])
            .frame(width: 200, height: 200)
            .background(Color.gray.opacity(0.1))
    }

    #Preview("Speaking") {
        AvatarView(stateType: .speaking, isConnected: true, hasMessage: true)
            .environment(\.theme, allThemes[0])
            .frame(width: 200, height: 200)
            .background(Color.gray.opacity(0.1))
    }

    #Preview("Error + Message") {
        AvatarView(stateType: .error, isConnected: false, hasMessage: true)
            .environment(\.theme, allThemes[0])
            .frame(width: 200, height: 200)
            .background(Color.gray.opacity(0.1))
    }

    #Preview("Wants to Speak") {
        AvatarView(stateType: .wantsToSpeak, isConnected: true, hasMessage: false)
            .environment(\.theme, allThemes[0])
            .frame(width: 200, height: 200)
            .background(Color.gray.opacity(0.1))
    }
#endif
