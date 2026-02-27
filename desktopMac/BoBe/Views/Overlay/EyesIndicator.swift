import SwiftUI

/// Eye expression dispatcher for the avatar.
/// Individual eye states are in `EyeExpressions.swift`.
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
        switch state {
        case .error:      ErrorEyes()
        case .loading:    AttentiveEyes()
        case .idle:       idleEyes
        case .capturing:  CapturingEyes()
        case .thinking:   ThinkingEyes()
        case .speaking:   SpeakingEyes()
        case .wantsToSpeak: EagerEyes()
        }
    }

    @ViewBuilder
    private var idleEyes: some View {
        if chatOpen || warmupActive { AttentiveEyes() } else { SleepingEyes() }
    }
}

// MARK: - Sleeping Eyes (curved arcs ◡ ◡)

struct SleepingEyes: View {
    @Environment(\.theme) private var theme

    var body: some View {
        ZStack {
            SleepArc()
                .stroke(theme.colors.text, style: StrokeStyle(lineWidth: 2, lineCap: .round))
                .frame(width: 10, height: 6)
                .offset(x: -9)
                .opacity(0.6)
            SleepArc()
                .stroke(theme.colors.text, style: StrokeStyle(lineWidth: 2, lineCap: .round))
                .frame(width: 10, height: 6)
                .offset(x: 9)
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

struct ErrorEyes: View {
    @State private var frownVisible = true
    @Environment(\.theme) private var theme

    var body: some View {
        ZStack {
            XMark()
                .stroke(theme.colors.primary, style: StrokeStyle(lineWidth: 2, lineCap: .round))
                .frame(width: 8, height: 8)
                .offset(x: -9, y: -1)
            XMark()
                .stroke(theme.colors.primary, style: StrokeStyle(lineWidth: 2, lineCap: .round))
                .frame(width: 8, height: 8)
                .offset(x: 9, y: -1)
            FrownArc()
                .stroke(theme.colors.text, style: StrokeStyle(lineWidth: 1.5, lineCap: .round))
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
        path.move(to: .zero)
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
