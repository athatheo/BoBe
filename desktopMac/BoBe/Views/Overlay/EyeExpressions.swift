import SwiftUI

// MARK: - Reusable Eye Unit

/// Single eye composed of outline + sclera + iris + pupil + optional highlight.
struct EyeUnit: View {
    let theme: ThemeColors
    var irisOffset: CGPoint = .zero
    var showHighlight: Bool = false
    var highlightSize: CGFloat = 2
    var highlightOffset: CGPoint = CGPoint(x: -1.5, y: -1.5)
    var outlineSize: CGSize = CGSize(width: 13, height: 11)
    var irisSize: CGFloat = 6
    var pupilSize: CGFloat = 2.4

    var body: some View {
        ZStack {
            Ellipse()
                .stroke(theme.avatarEyeOutline, lineWidth: 1)
                .frame(width: outlineSize.width, height: outlineSize.height)
            Ellipse()
                .fill(.white)
                .frame(width: outlineSize.width - 2, height: outlineSize.height - 2)
            Circle()
                .fill(theme.avatarIris)
                .frame(width: irisSize, height: irisSize)
                .offset(x: irisOffset.x, y: irisOffset.y)
            Circle()
                .fill(theme.text)
                .frame(width: pupilSize, height: pupilSize)
                .offset(x: irisOffset.x, y: irisOffset.y)
            if showHighlight {
                Circle()
                    .fill(.white)
                    .frame(width: highlightSize, height: highlightSize)
                    .offset(x: highlightOffset.x, y: highlightOffset.y)
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
            ViewfinderCorners(opacity: bracketOpacity, color: theme.colors.text)

            EyeUnit(theme: theme.colors, irisOffset: CGPoint(x: pupilOffset, y: 0),
                    outlineSize: CGSize(width: 11, height: 10), irisSize: 5, pupilSize: 2)
                .offset(x: -8)
            EyeUnit(theme: theme.colors, irisOffset: CGPoint(x: pupilOffset, y: 0),
                    outlineSize: CGSize(width: 11, height: 10), irisSize: 5, pupilSize: 2)
                .offset(x: 8)

            Rectangle()
                .fill(theme.colors.secondary.opacity(0.4))
                .frame(width: 28, height: 1)
                .offset(y: scanOffset)
        }
        .onAppear {
            withAnimation(.easeInOut(duration: 2.5).repeatForever(autoreverses: true)) { pupilOffset = 1.5 }
            withAnimation(.easeInOut(duration: 2).repeatForever(autoreverses: true)) { bracketOpacity = 0.8 }
            withAnimation(.linear(duration: 3).repeatForever(autoreverses: true)) { scanOffset = 4 }
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
                EyeUnit(theme: theme.colors, irisOffset: CGPoint(x: 0, y: -1))
                EyeUnit(theme: theme.colors, irisOffset: CGPoint(x: 0, y: -1))
            }
            .offset(y: lookUpOffset)
            Ellipse()
                .stroke(theme.colors.avatarMouth, lineWidth: 2)
                .frame(width: 5, height: 4)
        }
        .onAppear {
            withAnimation(.easeInOut(duration: 2).repeatForever(autoreverses: true)) { lookUpOffset = 0 }
        }
    }
}

// MARK: - Speaking Eyes

struct SpeakingEyes: View {
    @State private var mouthScaleY: CGFloat = 1.0
    @Environment(\.theme) private var theme

    var body: some View {
        VStack(spacing: 2) {
            HStack(spacing: 8) {
                EyeUnit(theme: theme.colors, showHighlight: true)
                EyeUnit(theme: theme.colors, showHighlight: true)
            }
            Ellipse()
                .fill(theme.colors.avatarMouth)
                .frame(width: 8, height: 4)
                .scaleEffect(y: mouthScaleY)
        }
        .task {
            let frames: [CGFloat] = [1, 1.75, 0.75, 1.5, 1]
            var i = 0
            while !Task.isCancelled {
                try? await Task.sleep(for: .milliseconds(125))
                withAnimation(.easeInOut(duration: 0.1)) { mouthScaleY = frames[i % frames.count] }
                i &+= 1
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
                    .stroke(theme.colors.text.opacity(0.6), style: StrokeStyle(lineWidth: 2, lineCap: .round))
                    .frame(width: 10, height: 3)
                EyebrowShape()
                    .stroke(theme.colors.text.opacity(0.6), style: StrokeStyle(lineWidth: 2, lineCap: .round))
                    .frame(width: 10, height: 3)
            }
            .offset(y: browOffset)
            HStack(spacing: 8) {
                EyeUnit(theme: theme.colors, irisOffset: CGPoint(x: 0, y: -0.5),
                        showHighlight: true, highlightSize: 2.4,
                        highlightOffset: CGPoint(x: -2, y: -2),
                        outlineSize: CGSize(width: 13, height: 12),
                        irisSize: 7, pupilSize: 3)
                EyeUnit(theme: theme.colors, irisOffset: CGPoint(x: 0, y: -0.5),
                        showHighlight: true, highlightSize: 2.4,
                        highlightOffset: CGPoint(x: -2, y: -2),
                        outlineSize: CGSize(width: 13, height: 12),
                        irisSize: 7, pupilSize: 3)
            }
            SmilePath()
                .stroke(theme.colors.avatarMouth, style: StrokeStyle(lineWidth: 2, lineCap: .round))
                .frame(width: 10, height: 4)
        }
        .onAppear {
            withAnimation(.easeInOut(duration: 1).repeatForever(autoreverses: true)) { browOffset = -1 }
        }
    }
}

// MARK: - Attentive Eyes

struct AttentiveEyes: View {
    @State private var bobOffset: CGFloat = 0
    @Environment(\.theme) private var theme

    var body: some View {
        HStack(spacing: 8) {
            EyeUnit(theme: theme.colors, showHighlight: true)
            EyeUnit(theme: theme.colors, showHighlight: true)
        }
        .offset(y: bobOffset)
        .onAppear {
            withAnimation(.easeInOut(duration: 2).repeatForever(autoreverses: true)) { bobOffset = 1 }
        }
    }
}

// MARK: - Shared Shapes

private struct ViewfinderCorners: View {
    let opacity: Double
    let color: Color

    var body: some View {
        let style = StrokeStyle(lineWidth: 1.5, lineCap: .round, lineJoin: .round)
        ZStack {
            cornerPath(moves: [(0, 4), (0, 0), (4, 0)]).stroke(color.opacity(opacity), style: style).offset(x: -15, y: -9)
            cornerPath(moves: [(0, 0), (4, 0), (4, 4)]).stroke(color.opacity(opacity), style: style).offset(x: 11, y: -9)
            cornerPath(moves: [(0, 0), (0, 4), (4, 4)]).stroke(color.opacity(opacity), style: style).offset(x: -15, y: 5)
            cornerPath(moves: [(4, 4), (0, 4), (0, 0)]).stroke(color.opacity(opacity), style: style).offset(x: 11, y: 5)
        }
    }

    private func cornerPath(moves: [(CGFloat, CGFloat)]) -> Path {
        Path { p in
            guard let first = moves.first else { return }
            p.move(to: CGPoint(x: first.0, y: first.1))
            for pt in moves.dropFirst() { p.addLine(to: CGPoint(x: pt.0, y: pt.1)) }
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
