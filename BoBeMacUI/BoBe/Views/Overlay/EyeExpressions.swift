import SwiftUI

// MARK: - Eye Unit

struct EyeUnit: View {
    let theme: ThemeColors
    var irisOffset: CGPoint = .zero
    var showHighlight: Bool = false
    var highlightSize: CGFloat = 2
    var highlightOffset = CGPoint(x: -1.5, y: -1.5)
    var outlineSize = CGSize(width: 13, height: 11)
    var irisSize: CGFloat = 6
    var pupilSize: CGFloat = 2.4

    var body: some View {
        ZStack {
            Ellipse()
                .stroke(self.theme.avatarEyeOutline, lineWidth: 1)
                .frame(width: self.outlineSize.width, height: self.outlineSize.height)
            Ellipse()
                .fill(.white)
                .frame(width: self.outlineSize.width - 2, height: self.outlineSize.height - 2)
            Circle()
                .fill(self.theme.avatarIris)
                .frame(width: self.irisSize, height: self.irisSize)
                .offset(x: self.irisOffset.x, y: self.irisOffset.y)
            Circle()
                .fill(self.theme.text)
                .frame(width: self.pupilSize, height: self.pupilSize)
                .offset(x: self.irisOffset.x, y: self.irisOffset.y)
            if self.showHighlight {
                Circle()
                    .fill(.white)
                    .frame(width: self.highlightSize, height: self.highlightSize)
                    .offset(x: self.highlightOffset.x, y: self.highlightOffset.y)
            }
        }
    }
}

// MARK: - Capturing Eyes

struct CapturingEyes: View {
    @State private var pupilOffset: CGFloat = -1.5
    @State private var bracketOpacity: Double = 0.4
    @State private var scanOffset: CGFloat = -4
    @Environment(\.theme) private var theme

    var body: some View {
        ZStack {
            HStack(spacing: 5) {
                EyeUnit(
                    theme: self.theme.colors,
                    irisOffset: CGPoint(x: self.pupilOffset, y: 0),
                    outlineSize: CGSize(width: 11, height: 10),
                    irisSize: 5,
                    pupilSize: 2
                )
                EyeUnit(
                    theme: self.theme.colors,
                    irisOffset: CGPoint(x: self.pupilOffset, y: 0),
                    outlineSize: CGSize(width: 11, height: 10),
                    irisSize: 5,
                    pupilSize: 2
                )
            }
            .overlay {
                ViewfinderCorners(opacity: self.bracketOpacity, color: self.theme.colors.text)
                    .frame(width: 31, height: 14)
            }

            Rectangle()
                .fill(self.theme.colors.secondary.opacity(0.4))
                .frame(width: 28, height: 1)
                .offset(y: self.scanOffset)
        }
        // `.task` rather than `.onAppear` so the structured-concurrency
        // cancellation cleanly tears down the implicit animations when the
        // view leaves the hierarchy (state transition to a different eye
        // variant). SwiftUI cancels animations on view-removal regardless,
        // but `.task` is the idiomatic lifecycle binding.
        .task {
            guard OverlayMotionRuntime.shouldAnimate else { return }
            withAnimation(.easeInOut(duration: 2.5).repeatForever(autoreverses: true)) { self.pupilOffset = 1.5 }
            withAnimation(.easeInOut(duration: 2).repeatForever(autoreverses: true)) { self.bracketOpacity = 0.8 }
            withAnimation(.linear(duration: 3).repeatForever(autoreverses: true)) { self.scanOffset = 4 }
        }
    }
}

// MARK: - Thinking Eyes

struct ThinkingEyes: View {
    @State private var lookUpOffset: CGFloat = -1
    @Environment(\.theme) private var theme

    var body: some View {
        VStack(spacing: 2) {
            HStack(spacing: 8) {
                EyeUnit(theme: self.theme.colors, irisOffset: CGPoint(x: 0, y: -1))
                EyeUnit(theme: self.theme.colors, irisOffset: CGPoint(x: 0, y: -1))
            }
            .offset(y: self.lookUpOffset)
            Ellipse()
                .stroke(self.theme.colors.avatarMouth, lineWidth: 2)
                .frame(width: 5, height: 4)
        }
        .task {
            guard OverlayMotionRuntime.shouldAnimate else { return }
            withAnimation(.easeInOut(duration: 2).repeatForever(autoreverses: true)) { self.lookUpOffset = 0 }
        }
    }
}

// MARK: - Mouth shape

/// Custom mouth shape — two curved lips meeting in the middle that open
/// vertically as `openness` increases. Closed (`openness ≈ 0`) reads as a
/// thin horizontal line; fully open (`openness = 1`) reads as an almond
/// shape with visible cavity. Asymmetric on purpose: the lower lip bows
/// further than the upper, giving the mouth character (matches how mouths
/// actually open — the jaw drops, the upper lip barely moves).
struct OpenMouthShape: Shape {
    /// 0 = closed, 1 = wide open.
    var openness: CGFloat

    var animatableData: CGFloat {
        get { self.openness }
        set { self.openness = newValue }
    }

    func path(in rect: CGRect) -> Path {
        var path = Path()
        let cx = rect.midX
        let cy = rect.midY
        let halfW = rect.width * 0.5
        // Vertical opening blends from ~0.4pt (mostly closed) to a
        // capped fraction of the rect height. We never let the mouth
        // exceed its bounding rect or it'd clip against the eyes.
        let openHalf = 0.4 + max(0, min(1, self.openness)) * (rect.height * 0.45)

        // Left corner of mouth.
        path.move(to: CGPoint(x: cx - halfW, y: cy))
        // Upper lip — bows up subtly (about 65% of the opening height).
        path.addQuadCurve(
            to: CGPoint(x: cx + halfW, y: cy),
            control: CGPoint(x: cx, y: cy - openHalf * 0.65)
        )
        // Lower lip — bows down further than the upper, asymmetric.
        path.addQuadCurve(
            to: CGPoint(x: cx - halfW, y: cy),
            control: CGPoint(x: cx, y: cy + openHalf)
        )
        path.closeSubpath()
        return path
    }
}

// MARK: - Speaking Eyes

/// Avatar face while BoBe is speaking. Mouth uses a real `OpenMouthShape`
/// rather than a scaled capsule:
///   • dark cavity fill underneath (visible as the mouth opens)
///   • softer secondary-tinted lip outline on top
///   • optional tongue ellipse hint when the envelope crests
///
/// Drive: `VoicePipeline.ttsOutputLevel` (live playback envelope) +
/// a baseline beat so the mouth keeps breathing through quiet syllables
/// rather than freezing shut. Two animation channels — a fast envelope
/// follower for attack and a slow beat for the idle breath.
struct SpeakingEyes: View {
    private let pipeline = VoicePipeline.shared
    @State private var displayLevel: CGFloat = 0
    @State private var fallbackBeat: CGFloat = 0.3
    @Environment(\.theme) private var theme

    /// Combine baseline beat and live envelope into a single openness
    /// signal in 0…1, biased so loud syllables open the mouth wide while
    /// quiet moments still show a soft breath.
    private var openness: CGFloat {
        let beatComponent = self.fallbackBeat * 0.20
        let envelopeComponent = self.displayLevel * 1.2
        return max(0, min(1, beatComponent + envelopeComponent))
    }

    var body: some View {
        VStack(spacing: 2) {
            HStack(spacing: 8) {
                EyeUnit(theme: self.theme.colors, showHighlight: true)
                EyeUnit(theme: self.theme.colors, showHighlight: true)
            }

            ZStack {
                // Dark cavity — the inside of the mouth. Sits behind the
                // lip outline so as the mouth opens the dark interior is
                // what reads visually, not just a moving line.
                OpenMouthShape(openness: self.openness)
                    .fill(self.theme.colors.text.opacity(0.85))
                    .frame(width: 14, height: 9)

                // Lip outline in the brand mouth tint.
                OpenMouthShape(openness: self.openness)
                    .stroke(
                        self.theme.colors.avatarMouth,
                        style: StrokeStyle(lineWidth: 1.1, lineCap: .round, lineJoin: .round)
                    )
                    .frame(width: 14, height: 9)

                // Tongue hint — only when really open, gives the mouth
                // character on loud syllables. Tinted with the secondary
                // accent so it reads as warm/alive against the dark
                // cavity.
                if self.openness > 0.42 {
                    Ellipse()
                        .fill(self.theme.colors.secondary.opacity(0.55))
                        .frame(width: 5.5, height: 1.6)
                        .offset(y: 1.6)
                        .transition(.opacity)
                }
            }
            // Smoothed envelope tween. Linear at ~80ms matches the
            // RMS sample cadence (~21ms) + decay tick (16ms) so
            // consecutive updates flow into each other rather than each
            // restarting an easeOut curve mid-interpolation (which read
            // as judder). Longer than 80ms blurs syllable attacks; much
            // shorter and the value changes outpace the tween window.
            .animation(
                OverlayMotionRuntime.reduceMotion ? nil : .linear(duration: 0.08),
                value: self.displayLevel
            )
            .animation(
                OverlayMotionRuntime.reduceMotion ? nil : .easeInOut(duration: 0.45),
                value: self.fallbackBeat
            )
        }
        .onChange(of: self.pipeline.ttsOutputLevel) { _, newLevel in
            self.displayLevel = CGFloat(newLevel)
        }
        .task {
            guard OverlayMotionRuntime.shouldAnimate else {
                self.displayLevel = 0.4
                return
            }
            // Always-on breath beat — combines with the live envelope.
            // Cadence is intentionally irregular so the mouth doesn't
            // feel metronomic when there's no audio playing yet (i.e.
            // we just transitioned to .speaking but the player hasn't
            // started yet).
            let beats: [CGFloat] = [0.7, 0.3, 1.0, 0.5, 0.85, 0.2, 0.6, 0.95]
            var idx = 0
            while !Task.isCancelled {
                self.fallbackBeat = beats[idx % beats.count]
                idx &+= 1
                try? await Task.sleep(for: .milliseconds(180))
            }
        }
    }
}

// MARK: - Eager Eyes

struct EagerEyes: View {
    @State private var browOffset: CGFloat = 0
    @Environment(\.theme) private var theme

    var body: some View {
        VStack(spacing: 1) {
            HStack(spacing: 12) {
                EyebrowShape()
                    .stroke(self.theme.colors.text.opacity(0.6), style: StrokeStyle(lineWidth: 2, lineCap: .round))
                    .frame(width: 10, height: 3)
                EyebrowShape()
                    .stroke(self.theme.colors.text.opacity(0.6), style: StrokeStyle(lineWidth: 2, lineCap: .round))
                    .frame(width: 10, height: 3)
            }
            .offset(y: self.browOffset)
            HStack(spacing: 8) {
                EyeUnit(
                    theme: self.theme.colors,
                    irisOffset: CGPoint(x: 0, y: -0.5),
                    showHighlight: true,
                    highlightSize: 2.4,
                    highlightOffset: CGPoint(x: -2, y: -2),
                    outlineSize: CGSize(width: 13, height: 12),
                    irisSize: 7,
                    pupilSize: 3
                )
                EyeUnit(
                    theme: self.theme.colors,
                    irisOffset: CGPoint(x: 0, y: -0.5),
                    showHighlight: true,
                    highlightSize: 2.4,
                    highlightOffset: CGPoint(x: -2, y: -2),
                    outlineSize: CGSize(width: 13, height: 12),
                    irisSize: 7,
                    pupilSize: 3
                )
            }
            SmilePath()
                .stroke(self.theme.colors.avatarMouth, style: StrokeStyle(lineWidth: 2, lineCap: .round))
                .frame(width: 10, height: 4)
        }
        .task {
            guard OverlayMotionRuntime.shouldAnimate else { return }
            withAnimation(.easeInOut(duration: 1).repeatForever(autoreverses: true)) { self.browOffset = -1 }
        }
    }
}

// MARK: - Attentive Eyes

struct AttentiveEyes: View {
    @State private var irisNudge: CGFloat = 0
    @Environment(\.theme) private var theme

    var body: some View {
        HStack(spacing: 8) {
            EyeUnit(theme: self.theme.colors, irisOffset: CGPoint(x: 0, y: self.irisNudge), showHighlight: true)
            EyeUnit(theme: self.theme.colors, irisOffset: CGPoint(x: 0, y: self.irisNudge), showHighlight: true)
        }
        .task {
            guard OverlayMotionRuntime.shouldAnimate else { return }
            withAnimation(.easeInOut(duration: 2).repeatForever(autoreverses: true)) {
                self.irisNudge = -0.3
            }
        }
    }
}

// MARK: - Shared Shapes

private struct ViewfinderCorners: View {
    let opacity: Double
    let color: Color

    var body: some View {
        GeometryReader { proxy in
            let width = proxy.size.width
            let height = proxy.size.height
            let corner: CGFloat = 4
            let style = StrokeStyle(lineWidth: 1.5, lineCap: .round, lineJoin: .round)

            Path { p in
                p.move(to: CGPoint(x: 0, y: corner))
                p.addLine(to: CGPoint(x: 0, y: 0))
                p.addLine(to: CGPoint(x: corner, y: 0))

                p.move(to: CGPoint(x: width - corner, y: 0))
                p.addLine(to: CGPoint(x: width, y: 0))
                p.addLine(to: CGPoint(x: width, y: corner))

                p.move(to: CGPoint(x: 0, y: height - corner))
                p.addLine(to: CGPoint(x: 0, y: height))
                p.addLine(to: CGPoint(x: corner, y: height))

                p.move(to: CGPoint(x: width - corner, y: height))
                p.addLine(to: CGPoint(x: width, y: height))
                p.addLine(to: CGPoint(x: width, y: height - corner))
            }
            .stroke(self.color.opacity(self.opacity), style: style)
        }
    }
}

private struct EyebrowShape: Shape {
    func path(in rect: CGRect) -> Path {
        var p = Path()
        p.move(to: CGPoint(x: 0, y: rect.height))
        p.addQuadCurve(to: CGPoint(x: rect.width, y: rect.height), control: CGPoint(x: rect.width / 2, y: 0))
        return p
    }
}

private struct SmilePath: Shape {
    func path(in rect: CGRect) -> Path {
        var p = Path()
        p.move(to: CGPoint(x: 0, y: 0))
        p.addQuadCurve(to: CGPoint(x: rect.width, y: 0), control: CGPoint(x: rect.width / 2, y: rect.height))
        return p
    }
}

// MARK: - Previews

#if !SPM_BUILD
    #Preview("Eye Unit") {
        HStack(spacing: 20) {
            EyeUnit(theme: allThemes[0].colors)
            EyeUnit(theme: allThemes[0].colors, showHighlight: true)
            EyeUnit(theme: allThemes[0].colors, irisOffset: CGPoint(x: 2, y: -1))
        }
        .padding()
        .background(Color.gray.opacity(0.2))
    }

    #Preview("Capturing Eyes") {
        CapturingEyes()
            .environment(\.theme, allThemes[0])
            .padding()
            .background(Color.gray.opacity(0.2))
    }

    #Preview("Thinking Eyes") {
        ThinkingEyes()
            .environment(\.theme, allThemes[0])
            .padding()
            .background(Color.gray.opacity(0.2))
    }

    #Preview("Speaking Eyes") {
        SpeakingEyes()
            .environment(\.theme, allThemes[0])
            .padding()
            .background(Color.gray.opacity(0.2))
    }

    #Preview("Eager Eyes") {
        EagerEyes()
            .environment(\.theme, allThemes[0])
            .padding()
            .background(Color.gray.opacity(0.2))
    }

    #Preview("Attentive Eyes") {
        AttentiveEyes()
            .environment(\.theme, allThemes[0])
            .padding()
            .background(Color.gray.opacity(0.2))
    }
#endif
