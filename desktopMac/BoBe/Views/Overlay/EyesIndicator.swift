import SwiftUI

/// Eye expressions for the avatar, matching the exact SVG from EyesIndicator.tsx.
/// The SVG viewBox is "0 0 36 24" — we scale to fit in the avatar face.
struct EyesIndicator: View {
    let state: BobeStateType
    var chatOpen: Bool = false

    @State private var warmupActive = true
    private let warmupDuration: TimeInterval = 5 * 60

    @Environment(\.theme) private var theme

    var body: some View {
        eyeView
            .frame(width: 40, height: 28)
            .task {
                try? await Task.sleep(for: .seconds(warmupDuration))
                warmupActive = false
            }
    }

    @ViewBuilder
    private var eyeView: some View {
        switch effectiveState {
        case .error:
            ErrorEyes()
        case .loading:
            AttentiveEyes()
        case .idle:
            if chatOpen || warmupActive {
                AttentiveEyes()
            } else {
                SleepingEyes()
            }
        case .capturing:
            CapturingEyes()
        case .thinking:
            ThinkingEyes()
        case .speaking:
            SpeakingEyes()
        case .wantsToSpeak:
            EagerEyes()
        }
    }

    private var effectiveState: BobeStateType { state }
}

// MARK: - SVG-scaled canvas helper

/// All eye states use SVG viewBox="0 0 36 24". This scales them into SwiftUI.
private struct EyeCanvas<Content: View>: View {
    let viewBoxWidth: CGFloat
    let viewBoxHeight: CGFloat
    @ViewBuilder var content: Content

    init(
        width: CGFloat = 36,
        height: CGFloat = 24,
        @ViewBuilder content: () -> Content
    ) {
        self.viewBoxWidth = width
        self.viewBoxHeight = height
        self.content = content()
    }

    var body: some View {
        Canvas { context, size in
            // We draw using SwiftUI shapes inside ZStack instead
        }
        // Use ZStack with GeometryReader for SVG-like positioning
        .overlay {
            GeometryReader { geo in
                let sx = geo.size.width / viewBoxWidth
                let sy = geo.size.height / viewBoxHeight
                let s = min(sx, sy)
                content
                    .scaleEffect(s, anchor: .topLeading)
                    .frame(width: viewBoxWidth, height: viewBoxHeight, alignment: .topLeading)
            }
        }
    }
}

// MARK: - Sleeping Eyes (curved arcs ◡ ◡)
// SVG: <path d="M4 10 Q9 14, 14 10"> and <path d="M22 10 Q27 14, 32 10">

struct SleepingEyes: View {
    @Environment(\.theme) private var theme

    var body: some View {
        ZStack {
            // Left eye arc
            SleepArc()
                .stroke(theme.colors.text, style: StrokeStyle(lineWidth: 2, lineCap: .round))
                .frame(width: 10, height: 6)
                .offset(x: -9, y: 0)
                .opacity(0.6)

            // Right eye arc
            SleepArc()
                .stroke(theme.colors.text, style: StrokeStyle(lineWidth: 2, lineCap: .round))
                .frame(width: 10, height: 6)
                .offset(x: 9, y: 0)
                .opacity(0.6)
        }
    }
}

private struct SleepArc: Shape {
    func path(in rect: CGRect) -> Path {
        var path = Path()
        path.move(to: CGPoint(x: 0, y: 0))
        path.addQuadCurve(
            to: CGPoint(x: rect.width, y: 0),
            control: CGPoint(x: rect.width / 2, y: rect.height)
        )
        return path
    }
}

// MARK: - Error Eyes (X × X with frown)
// Terracotta X marks with a blinking frown mouth

struct ErrorEyes: View {
    @State private var frownVisible = true
    @Environment(\.theme) private var theme

    private var xColor: Color { theme.colors.primary }
    private var frownColor: Color { theme.colors.text }

    var body: some View {
        ZStack {
            // Left X
            XMark()
                .stroke(xColor, style: StrokeStyle(lineWidth: 2, lineCap: .round))
                .frame(width: 8, height: 8)
                .offset(x: -9, y: -1)

            // Right X
            XMark()
                .stroke(xColor, style: StrokeStyle(lineWidth: 2, lineCap: .round))
                .frame(width: 8, height: 8)
                .offset(x: 9, y: -1)

            // Frown mouth
            FrownArc()
                .stroke(frownColor, style: StrokeStyle(lineWidth: 1.5, lineCap: .round))
                .frame(width: 8, height: 4)
                .offset(y: 8)
                .opacity(frownVisible ? 1 : 0.3)
        }
        .task {
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(1.5))
                withAnimation(.easeInOut(duration: 0.3)) { frownVisible.toggle() }
                try? await Task.sleep(for: .seconds(0.5))
                withAnimation(.easeInOut(duration: 0.3)) { frownVisible.toggle() }
            }
        }
    }
}

private struct XMark: Shape {
    func path(in rect: CGRect) -> Path {
        var path = Path()
        path.move(to: CGPoint(x: 0, y: 0))
        path.addLine(to: CGPoint(x: rect.width, y: rect.height))
        path.move(to: CGPoint(x: rect.width, y: 0))
        path.addLine(to: CGPoint(x: 0, y: rect.height))
        return path
    }
}

private struct FrownArc: Shape {
    func path(in rect: CGRect) -> Path {
        var path = Path()
        path.move(to: CGPoint(x: 0, y: rect.height))
        path.addQuadCurve(
            to: CGPoint(x: rect.width, y: rect.height),
            control: CGPoint(x: rect.width / 2, y: 0)
        )
        return path
    }
}

// MARK: - Capturing Eyes (with viewfinder corners)
// SVG ellipses: cx=12,cy=12,rx=5.5,ry=5 and cx=28,cy=12,rx=5.5,ry=5
// Viewfinder: 4 corner paths, scan line

struct CapturingEyes: View {
    @State private var pupilOffset: CGFloat = -1.5
    @State private var bracketOpacity: Double = 0.4
    @State private var scanOffset: CGFloat = -4

    @Environment(\.theme) private var theme

    var body: some View {
        ZStack {
            // Viewfinder corners
            ViewfinderCorners(opacity: bracketOpacity, color: theme.colors.text)

            // Left eye
            CapturingEyeUnit(pupilOffset: pupilOffset, theme: theme.colors)
                .offset(x: -8, y: 0)

            // Right eye
            CapturingEyeUnit(pupilOffset: pupilOffset, theme: theme.colors)
                .offset(x: 8, y: 0)

            // Scan line
            Rectangle()
                .fill(theme.colors.secondary.opacity(0.4))
                .frame(width: 28, height: 1)
                .offset(y: scanOffset)
        }
        .onAppear {
            withAnimation(.easeInOut(duration: 2.5).repeatForever(autoreverses: true)) {
                pupilOffset = 1.5
            }
            withAnimation(.easeInOut(duration: 2).repeatForever(autoreverses: true)) {
                bracketOpacity = 0.8
            }
            withAnimation(.linear(duration: 3).repeatForever(autoreverses: true)) {
                scanOffset = 4
            }
        }
    }
}

private struct CapturingEyeUnit: View {
    let pupilOffset: CGFloat
    let theme: ThemeColors

    var body: some View {
        ZStack {
            // Eye outline
            Ellipse()
                .stroke(theme.avatarEyeOutline, lineWidth: 1)
                .frame(width: 11, height: 10)
            // Sclera
            Ellipse()
                .fill(.white)
                .frame(width: 9, height: 8)
            // Iris
            Circle()
                .fill(theme.avatarIris)
                .frame(width: 5, height: 5)
                .offset(x: pupilOffset)
            // Pupil
            Circle()
                .fill(theme.text)
                .frame(width: 2, height: 2)
                .offset(x: pupilOffset)
        }
    }
}

private struct ViewfinderCorners: View {
    let opacity: Double
    let color: Color

    var body: some View {
        ZStack {
            // Top-left: M2 6 L2 2 L6 2
            Path { p in
                p.move(to: CGPoint(x: 0, y: 4))
                p.addLine(to: CGPoint(x: 0, y: 0))
                p.addLine(to: CGPoint(x: 4, y: 0))
            }
            .stroke(color.opacity(opacity), style: StrokeStyle(lineWidth: 1.5, lineCap: .round, lineJoin: .round))
            .offset(x: -15, y: -9)

            // Top-right: M34 2 L38 2 L38 6
            Path { p in
                p.move(to: CGPoint(x: 0, y: 0))
                p.addLine(to: CGPoint(x: 4, y: 0))
                p.addLine(to: CGPoint(x: 4, y: 4))
            }
            .stroke(color.opacity(opacity), style: StrokeStyle(lineWidth: 1.5, lineCap: .round, lineJoin: .round))
            .offset(x: 11, y: -9)

            // Bottom-left: M2 18 L2 22 L6 22
            Path { p in
                p.move(to: CGPoint(x: 0, y: 0))
                p.addLine(to: CGPoint(x: 0, y: 4))
                p.addLine(to: CGPoint(x: 4, y: 4))
            }
            .stroke(color.opacity(opacity), style: StrokeStyle(lineWidth: 1.5, lineCap: .round, lineJoin: .round))
            .offset(x: -15, y: 5)

            // Bottom-right: M34 22 L38 22 L38 18
            Path { p in
                p.move(to: CGPoint(x: 4, y: 4))
                p.addLine(to: CGPoint(x: 0, y: 4))
                p.addLine(to: CGPoint(x: 0, y: 0))  // actually goes up: M34 22 L38 22 L38 18
            }
            .stroke(color.opacity(opacity), style: StrokeStyle(lineWidth: 1.5, lineCap: .round, lineJoin: .round))
            .offset(x: 11, y: 5)
        }
    }
}

// MARK: - Thinking Eyes (looking up with :o mouth)
// SVG: eyes at (9,9) and (27,9), iris offset y=-1, mouth ellipse at (18,19)

struct ThinkingEyes: View {
    @State private var lookUpOffset: CGFloat = -1

    @Environment(\.theme) private var theme

    var body: some View {
        VStack(spacing: 2) {
            HStack(spacing: 8) {
                ThinkingEyeUnit(theme: theme.colors)
                ThinkingEyeUnit(theme: theme.colors)
            }
            .offset(y: lookUpOffset)

            // Thinking mouth — small "o" shape
            Ellipse()
                .stroke(theme.colors.avatarMouth, lineWidth: 2)
                .frame(width: 5, height: 4)
        }
        .onAppear {
            withAnimation(.easeInOut(duration: 2).repeatForever(autoreverses: true)) {
                lookUpOffset = 0
            }
        }
    }
}

private struct ThinkingEyeUnit: View {
    let theme: ThemeColors

    var body: some View {
        ZStack {
            // Eye outline
            Ellipse()
                .stroke(theme.avatarEyeOutline, lineWidth: 1)
                .frame(width: 13, height: 11)
            // Sclera
            Ellipse()
                .fill(.white)
                .frame(width: 11, height: 9)
            // Iris (looking up: y offset -1)
            Circle()
                .fill(theme.avatarIris)
                .frame(width: 6, height: 6)
                .offset(y: -1)
            // Pupil
            Circle()
                .fill(theme.text)
                .frame(width: 2.4, height: 2.4)
                .offset(y: -1)
        }
    }
}

// MARK: - Speaking Eyes (with animated mouth)
// SVG: eyes at (9,9) and (27,9), highlights, animated mouth ellipse

struct SpeakingEyes: View {
    @State private var mouthScaleY: CGFloat = 1.0
    @Environment(\.theme) private var theme

    var body: some View {
        VStack(spacing: 2) {
            HStack(spacing: 8) {
                SpeakingEyeUnit(theme: theme.colors)
                SpeakingEyeUnit(theme: theme.colors)
            }

            // Animated talking mouth
            Ellipse()
                .fill(theme.colors.avatarMouth)
                .frame(width: 8, height: 4)
                .scaleEffect(y: mouthScaleY)
        }
        .onAppear {
            animateMouth()
        }
    }

    private func animateMouth() {
        Task { @MainActor in
            let keyframes: [CGFloat] = [1, 1.75, 0.75, 1.5, 1]
            var idx = 0
            while !Task.isCancelled {
                try? await Task.sleep(for: .milliseconds(125))
                withAnimation(.easeInOut(duration: 0.1)) {
                    mouthScaleY = keyframes[idx % keyframes.count]
                }
                idx += 1
                if idx >= keyframes.count * 10 { idx = 0 }
            }
        }
    }
}

private struct SpeakingEyeUnit: View {
    let theme: ThemeColors

    var body: some View {
        ZStack {
            // Eye outline
            Ellipse()
                .stroke(theme.avatarEyeOutline, lineWidth: 1)
                .frame(width: 13, height: 11)
            // Sclera
            Ellipse()
                .fill(.white)
                .frame(width: 11, height: 9)
            // Iris
            Circle()
                .fill(theme.avatarIris)
                .frame(width: 6, height: 6)
            // Pupil
            Circle()
                .fill(theme.text)
                .frame(width: 2.4, height: 2.4)
            // Highlight
            Circle()
                .fill(.white)
                .frame(width: 2, height: 2)
                .offset(x: -1.5, y: -1.5)
        }
    }
}

// MARK: - Eager Eyes (excited, wants to speak — eyebrows + smile)
// SVG: eyebrows d="M4 4 Q9 2, 14 4", eyes at (9,12)+(27,12), smile at bottom

struct EagerEyes: View {
    @State private var browOffset: CGFloat = 0
    @Environment(\.theme) private var theme

    var body: some View {
        VStack(spacing: 1) {
            // Raised eyebrows
            HStack(spacing: 12) {
                EyebrowShape()
                    .stroke(theme.colors.text.opacity(0.6),
                            style: StrokeStyle(lineWidth: 2, lineCap: .round))
                    .frame(width: 10, height: 3)
                EyebrowShape()
                    .stroke(theme.colors.text.opacity(0.6),
                            style: StrokeStyle(lineWidth: 2, lineCap: .round))
                    .frame(width: 10, height: 3)
            }
            .offset(y: browOffset)

            HStack(spacing: 8) {
                EagerEyeUnit(theme: theme.colors)
                EagerEyeUnit(theme: theme.colors)
            }

            // Happy smile — d="M14 23 Q18 26, 22 23"
            SmilePath()
                .stroke(theme.colors.avatarMouth,
                        style: StrokeStyle(lineWidth: 2, lineCap: .round))
                .frame(width: 10, height: 4)
        }
        .onAppear {
            withAnimation(.easeInOut(duration: 1).repeatForever(autoreverses: true)) {
                browOffset = -1
            }
        }
    }
}

private struct EyebrowShape: Shape {
    func path(in rect: CGRect) -> Path {
        var p = Path()
        p.move(to: CGPoint(x: 0, y: rect.height))
        p.addQuadCurve(
            to: CGPoint(x: rect.width, y: rect.height),
            control: CGPoint(x: rect.width / 2, y: 0)
        )
        return p
    }
}

private struct SmilePath: Shape {
    func path(in rect: CGRect) -> Path {
        var p = Path()
        p.move(to: CGPoint(x: 0, y: 0))
        p.addQuadCurve(
            to: CGPoint(x: rect.width, y: 0),
            control: CGPoint(x: rect.width / 2, y: rect.height)
        )
        return p
    }
}

private struct EagerEyeUnit: View {
    let theme: ThemeColors

    var body: some View {
        ZStack {
            // Eye outline (larger — rx=6.5, ry=6)
            Ellipse()
                .stroke(theme.avatarEyeOutline, lineWidth: 1)
                .frame(width: 13, height: 12)
            // Sclera
            Ellipse()
                .fill(.white)
                .frame(width: 11, height: 10)
            // Iris (slightly up: cy=11.5 in 24h space)
            Circle()
                .fill(theme.avatarIris)
                .frame(width: 7, height: 7)
                .offset(y: -0.5)
            // Pupil
            Circle()
                .fill(theme.text)
                .frame(width: 3, height: 3)
                .offset(y: -0.5)
            // Highlight
            Circle()
                .fill(.white)
                .frame(width: 2.4, height: 2.4)
                .offset(x: -2, y: -2)
        }
    }
}

// MARK: - Attentive Eyes (forward-looking, gentle bobbing)
// SVG: eyes at (9,10) and (27,10), highlights

struct AttentiveEyes: View {
    @State private var bobOffset: CGFloat = 0
    @Environment(\.theme) private var theme

    var body: some View {
        HStack(spacing: 8) {
            AttentiveEyeUnit(theme: theme.colors)
            AttentiveEyeUnit(theme: theme.colors)
        }
        .offset(y: bobOffset)
        .onAppear {
            withAnimation(.easeInOut(duration: 2).repeatForever(autoreverses: true)) {
                bobOffset = 1
            }
        }
    }
}

private struct AttentiveEyeUnit: View {
    let theme: ThemeColors

    var body: some View {
        ZStack {
            // Eye outline
            Ellipse()
                .stroke(theme.avatarEyeOutline, lineWidth: 1)
                .frame(width: 13, height: 11)
            // Sclera
            Ellipse()
                .fill(.white)
                .frame(width: 11, height: 9)
            // Iris
            Circle()
                .fill(theme.avatarIris)
                .frame(width: 6, height: 6)
            // Pupil
            Circle()
                .fill(theme.text)
                .frame(width: 2.4, height: 2.4)
            // Highlight
            Circle()
                .fill(.white)
                .frame(width: 2, height: 2)
                .offset(x: -1.5, y: -1.5)
        }
    }
}
