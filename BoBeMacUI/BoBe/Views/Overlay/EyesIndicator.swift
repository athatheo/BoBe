import SwiftUI

struct EyesIndicator: View {
    let state: BobeStateType
    var chatOpen: Bool = false

    @State private var warmupActive: Bool = EyesIndicator.computeWarmupActive()

    /// Process-lifetime anchor. The first time anyone reads this static, it
    /// captures the launch moment; every later read compares against the
    /// same anchor so we don't re-sleep 5 minutes every time the overlay
    /// is closed and reopened.
    private static let launchedAt: Date = .now
    private static let warmupDuration: TimeInterval = 5 * 60

    /// Snapshot the warmup gate from the app-launch anchor. Anyone who
    /// opens the overlay 6 minutes after launch starts already-cooled.
    private static func computeWarmupActive() -> Bool {
        Date.now.timeIntervalSince(self.launchedAt) < self.warmupDuration
    }

    @Environment(\.theme) private var theme

    var body: some View {
        self.eyeView
            .frame(width: 40, height: 28)
            .task {
                // Only sleep for the *remaining* warmup time, not the full
                // 5 minutes — and bail immediately if the timer has already
                // elapsed during a prior overlay open.
                let remaining = Self.warmupDuration - Date.now.timeIntervalSince(Self.launchedAt)
                guard remaining > 0 else {
                    if self.warmupActive { self.warmupActive = false }
                    return
                }
                try? await Task.sleep(for: .seconds(remaining))
                if !Task.isCancelled { self.warmupActive = false }
            }
    }

    @ViewBuilder
    private var eyeView: some View {
        // Single source of truth: `state` is computed by the caller
        // (`OverlayView.avatarStateType`) which already folds in
        // `VoicePipeline.state == .speaking` and `ttsOutputLevel > 0`.
        // Don't duplicate the check here — it produced split-brain when
        // one source said speaking and the other said idle.
        switch self.state {
        case .error: ErrorEyes()
        case .loading: AttentiveEyes()
        case .idle: self.idleEyes
        case .capturing: CapturingEyes()
        case .thinking: ThinkingEyes()
        case .speaking: SpeakingEyes()
        case .wantsToSpeak: EagerEyes()
        case .shuttingDown: SleepingEyes()
        }
    }

    @ViewBuilder
    private var idleEyes: some View {
        if self.chatOpen || self.warmupActive { AttentiveEyes() } else { SleepingEyes() }
    }
}

// MARK: - Sleeping Eyes

struct SleepingEyes: View {
    @Environment(\.theme) private var theme

    var body: some View {
        ZStack {
            SleepArc()
                .stroke(self.theme.colors.text, style: StrokeStyle(lineWidth: 2, lineCap: .round))
                .frame(width: 10, height: 6)
                .offset(x: -9)
                .opacity(0.6)
            SleepArc()
                .stroke(self.theme.colors.text, style: StrokeStyle(lineWidth: 2, lineCap: .round))
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

// MARK: - Error Eyes

struct ErrorEyes: View {
    @State private var frownVisible = true
    @Environment(\.theme) private var theme

    var body: some View {
        ZStack {
            XMark()
                .stroke(self.theme.colors.primary, style: StrokeStyle(lineWidth: 2, lineCap: .round))
                .frame(width: 8, height: 8)
                .offset(x: -9, y: -1)
            XMark()
                .stroke(self.theme.colors.primary, style: StrokeStyle(lineWidth: 2, lineCap: .round))
                .frame(width: 8, height: 8)
                .offset(x: 9, y: -1)
            FrownArc()
                .stroke(self.theme.colors.text, style: StrokeStyle(lineWidth: 1.5, lineCap: .round))
                .frame(width: 8, height: 4)
                .offset(y: 8)
                .opacity(self.frownVisible ? 1 : 0.3)
        }
        .task {
            guard OverlayMotionRuntime.shouldAnimate else { return }
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(1.5))
                withAnimation(.easeInOut(duration: 0.3)) { self.frownVisible.toggle() }
                try? await Task.sleep(for: .seconds(0.5))
                withAnimation(.easeInOut(duration: 0.3)) { self.frownVisible.toggle() }
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

// MARK: - Previews

#if !SPM_BUILD
    #Preview("Idle") {
        EyesIndicator(state: .idle)
            .environment(\.theme, allThemes[0])
            .padding()
            .background(Color.gray.opacity(0.2))
    }

    #Preview("Thinking") {
        EyesIndicator(state: .thinking)
            .environment(\.theme, allThemes[0])
            .padding()
            .background(Color.gray.opacity(0.2))
    }

    #Preview("Speaking") {
        EyesIndicator(state: .speaking)
            .environment(\.theme, allThemes[0])
            .padding()
            .background(Color.gray.opacity(0.2))
    }

    #Preview("Error") {
        EyesIndicator(state: .error)
            .environment(\.theme, allThemes[0])
            .padding()
            .background(Color.gray.opacity(0.2))
    }

    #Preview("Sleeping") {
        EyesIndicator(state: .shuttingDown)
            .environment(\.theme, allThemes[0])
            .padding()
            .background(Color.gray.opacity(0.2))
    }
#endif
