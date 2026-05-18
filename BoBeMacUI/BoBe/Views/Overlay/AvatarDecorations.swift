import SwiftUI

// MARK: - Status Label

struct StatusLabel: View {
    let stateType: BobeStateType
    var textOverride: String?

    @State private var displayedText = ""
    @State private var targetText = ""
    @State private var isTyping = false
    @State private var showCursor = true
    @State private var lastShownAt: Date = .distantPast
    @State private var typewriterTask: Task<Void, Never>?
    @State private var delayTask: Task<Void, Never>?
    @Environment(\.theme) private var theme

    private let charDelay: TimeInterval = 0.04
    private let minDisplayTime: TimeInterval = 2.0

    var body: some View {
        Group {
            if !self.targetText.isEmpty || self.isTyping {
                HStack(spacing: 0) {
                    Text(self.displayedText)
                        .bobeTextStyle(.brandLabel)
                        .tracking(0.5)
                        .foregroundStyle(self.theme.colors.primary)

                    if self.isTyping || !self.displayedText.isEmpty {
                        Text("|")
                            .bobeTextStyle(.brandLabel)
                            .foregroundStyle(self.theme.colors.primary)
                            .opacity(self.showCursor ? 1 : 0)
                    }
                }
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
                .transition(.opacity.combined(with: .scale(scale: 0.9)))
                .zIndex(20)
            }
        }
        .animation(OverlayMotionRuntime.animation(for: .statusLabelTransition), value: self.displayedText)
        .onChange(of: self.stateType) { _, _ in
            self.scheduleLabelUpdate()
        }
        .task {
            self.startTypewriter(self.effectiveText(for: self.stateType))
            guard OverlayMotionRuntime.shouldAnimate else { return }
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(0.3))
                self.showCursor.toggle()
            }
        }
        .onChange(of: self.textOverride) { _, _ in
            self.scheduleLabelUpdate()
        }
        .onDisappear {
            self.typewriterTask?.cancel()
            self.delayTask?.cancel()
        }
    }

    /// Reconcile the visible label with the current `stateType` /
    /// `textOverride` pair. Respects `minDisplayTime` so labels don't
    /// flicker on rapid state churn — but the delayed update re-reads
    /// the *current* state at fire time so it can't apply a stale
    /// text. Also bails when the new text already matches what's
    /// displayed (e.g. state ping-ponged out and back to the same
    /// thing) and cancels any pending delayed update that would
    /// otherwise clobber the now-correct label.
    private func scheduleLabelUpdate() {
        let newText = self.effectiveText(for: self.stateType)
        guard newText != self.targetText else {
            // State matches what's already shown. Any pending delayed
            // update was scheduled against an older state — drop it so
            // it can't fire and clear a label that's now correct.
            self.delayTask?.cancel()
            self.delayTask = nil
            return
        }
        self.delayTask?.cancel()
        let elapsed = Date.now.timeIntervalSince(self.lastShownAt)
        if elapsed < self.minDisplayTime, !self.targetText.isEmpty {
            self.delayTask = Task { @MainActor in
                try? await Task.sleep(for: .seconds(self.minDisplayTime - elapsed))
                guard !Task.isCancelled else { return }
                // Re-evaluate at fire time — state may have changed
                // again since we scheduled this. The captured `newText`
                // from when we were scheduled may be stale.
                let currentText = self.effectiveText(for: self.stateType)
                guard currentText != self.targetText else { return }
                self.startTypewriter(currentText)
            }
        } else {
            self.startTypewriter(newText)
        }
    }

    private func startTypewriter(_ text: String) {
        self.typewriterTask?.cancel()
        self.targetText = text
        self.displayedText = ""
        self.lastShownAt = .now
        // Empty text means "hide the label" — skip the typewriter task
        // entirely. Setting isTyping=true for a zero-iteration loop
        // caused a brief frame where the pill rendered with no text but
        // a blinking cursor (`targetText.isEmpty || isTyping`).
        if text.isEmpty {
            self.isTyping = false
            return
        }
        self.isTyping = true
        self.typewriterTask = Task { @MainActor in
            for char in text {
                guard !Task.isCancelled else { return }
                self.displayedText.append(char)
                try? await Task.sleep(for: .seconds(self.charDelay))
            }
            self.isTyping = false
        }
    }

    private func effectiveText(for state: BobeStateType) -> String {
        if let override = textOverride, !override.isEmpty {
            return override
        }
        return self.labelText(for: state)
    }

    private func labelText(for state: BobeStateType) -> String {
        switch state {
        case .loading: L10n.tr("overlay.status.loading")
        case .idle: ""
        case .capturing: L10n.tr("overlay.status.capturing")
        case .thinking: L10n.tr("overlay.status.thinking")
        case .speaking: L10n.tr("overlay.status.speaking")
        case .wantsToSpeak: L10n.tr("overlay.status.wants_to_speak")
        case .error: L10n.tr("overlay.status.offline")
        case .shuttingDown: L10n.tr("overlay.status.shutting_down")
        }
    }
}

// MARK: - Thinking Numbers Ring

struct ThinkingNumbersRing: View {
    private let chars: [String] = ["1", "+", "0", "=", "π", "7", "%", "∑", "×", "2", "/", "9"]
    @Environment(\.theme) private var theme

    var body: some View {
        ZStack {
            ForEach(Array(self.chars.enumerated()), id: \.offset) { index, char in
                BubblingChar(
                    char: char,
                    color: self.theme.colors.primary,
                    index: index
                )
            }
        }
        .frame(width: 116, height: 116)
        .clipped()
    }
}

private struct BubblingChar: View {
    let char: String
    let color: Color
    let index: Int

    @State private var yOffset: CGFloat = 30
    @State private var charOpacity: Double = 0
    @State private var xPos: CGFloat = 0

    private static func randomX() -> CGFloat {
        CGFloat.random(in: -45 ... 45)
    }

    var body: some View {
        Text(self.char)
            .font(.system(size: 14, weight: .bold))
            .foregroundStyle(self.color)
            .frame(width: 16, height: 16)
            .offset(x: self.xPos, y: self.yOffset)
            .opacity(self.charOpacity)
            .task {
                guard OverlayMotionRuntime.shouldAnimate else {
                    self.charOpacity = 0
                    return
                }
                self.xPos = Self.randomX()
                let delay = Double(index) * 0.4
                try? await Task.sleep(for: .seconds(delay))
                guard !Task.isCancelled else { return }
                let duration = 2.5 + Double(self.index % 3) * 0.3
                while !Task.isCancelled {
                    self.xPos = Self.randomX()
                    self.yOffset = 30
                    self.charOpacity = 0
                    withAnimation(.easeIn(duration: duration * 0.2)) { self.charOpacity = 1 }
                    withAnimation(.easeOut(duration: duration)) { self.yOffset = -40 }
                    try? await Task.sleep(for: .seconds(duration * 0.8))
                    guard !Task.isCancelled else { return }
                    withAnimation(.easeOut(duration: duration * 0.2)) { self.charOpacity = 0 }
                    try? await Task.sleep(for: .seconds(duration * 0.2))
                }
            }
    }
}

// MARK: - Speaking Wave Ring

struct SpeakingWaveRing: View {
    private let barCount = 16
    @Environment(\.theme) private var theme

    var body: some View {
        ZStack {
            ForEach(0 ..< self.barCount, id: \.self) { index in
                SpeakingBar(
                    angle: Double(index) * (360.0 / Double(self.barCount)),
                    delay: Double(index) * 0.08,
                    color: self.theme.colors.secondary
                )
            }
        }
        .frame(width: 116, height: 116)
    }
}

private struct SpeakingBar: View {
    let angle: Double
    let delay: TimeInterval
    let color: Color

    @State private var scaleY: CGFloat = 0.3

    var body: some View {
        RoundedRectangle(cornerRadius: 2)
            .fill(self.color)
            .frame(width: 4, height: 12)
            .scaleEffect(y: self.scaleY)
            .offset(y: -52)
            .rotationEffect(.degrees(self.angle))
            .task {
                guard OverlayMotionRuntime.shouldAnimate else {
                    self.scaleY = 0.6
                    return
                }
                try? await Task.sleep(for: .seconds(self.delay))
                let frames: [CGFloat] = [0.3, 1.0, 0.5, 0.8, 0.3]
                var i = 0
                while !Task.isCancelled {
                    withAnimation(.easeInOut(duration: 0.12)) {
                        self.scaleY = frames[i % frames.count]
                    }
                    i += 1
                    try? await Task.sleep(for: .seconds(0.12))
                }
            }
    }
}

// MARK: - Attention Pulse

struct AttentionPulse: View {
    @State private var scale: CGFloat = 1.0
    @State private var opacity: Double = 0.7
    @Environment(\.theme) private var theme

    var body: some View {
        Circle()
            .stroke(self.theme.colors.primary, lineWidth: 3)
            .frame(width: 124, height: 124)
            .scaleEffect(self.scale)
            .opacity(self.opacity)
            // `.task` for the structured-concurrency lifecycle — SwiftUI
            // cancels animations on view-removal regardless, but `.task`
            // is the idiomatic match for the rest of the avatar's
            // animation primitives (CapturingEyes, ThinkingEyes, etc.).
            .task {
                guard OverlayMotionRuntime.shouldAnimate else {
                    self.scale = 1.04
                    self.opacity = 0.85
                    return
                }
                withAnimation(.easeInOut(duration: 1.5).repeatForever(autoreverses: true)) {
                    self.scale = 1.08
                    self.opacity = 1.0
                }
            }
    }
}

// MARK: - Previews

#if !SPM_BUILD
    #Preview("Status Labels") {
        VStack(spacing: 12) {
            StatusLabel(stateType: .thinking)
            StatusLabel(stateType: .capturing)
            StatusLabel(stateType: .error)
            StatusLabel(stateType: .wantsToSpeak)
            StatusLabel(stateType: .idle, textOverride: "Custom text")
        }
        .environment(\.theme, allThemes[0])
        .padding()
        .background(Color.gray.opacity(0.1))
    }

    #Preview("Thinking Numbers Ring") {
        ThinkingNumbersRing()
            .environment(\.theme, allThemes[0])
            .frame(width: 140, height: 140)
            .background(Color.gray.opacity(0.1))
    }

    #Preview("Speaking Wave Ring") {
        SpeakingWaveRing()
            .environment(\.theme, allThemes[0])
            .frame(width: 140, height: 140)
            .background(Color.gray.opacity(0.1))
    }

    #Preview("Attention Pulse") {
        AttentionPulse()
            .environment(\.theme, allThemes[0])
            .frame(width: 160, height: 160)
            .background(Color.gray.opacity(0.1))
    }
#endif
