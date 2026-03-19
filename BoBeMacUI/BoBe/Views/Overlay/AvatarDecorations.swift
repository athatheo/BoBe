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
            if !targetText.isEmpty || isTyping {
                HStack(spacing: 0) {
                    Text(displayedText)
                        .bobeTextStyle(.brandLabel)
                        .tracking(0.5)
                        .foregroundStyle(theme.colors.primary)

                    if isTyping || !displayedText.isEmpty {
                        Text("|")
                            .bobeTextStyle(.brandLabel)
                            .foregroundStyle(theme.colors.primary)
                            .opacity(showCursor ? 1 : 0)
                    }
                }
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
                .transition(.opacity.combined(with: .scale(scale: 0.9)))
                .zIndex(20)
            }
        }
        .animation(OverlayMotionRuntime.animation(for: .statusLabelTransition), value: displayedText)
        .onChange(of: stateType) { _, newState in
            let newText = effectiveText(for: newState)
            if newText != targetText {
                delayTask?.cancel()
                let elapsed = Date().timeIntervalSince(lastShownAt)
                if elapsed < minDisplayTime && !targetText.isEmpty {
                    delayTask = Task { @MainActor in
                        try? await Task.sleep(for: .seconds(minDisplayTime - elapsed))
                        guard !Task.isCancelled else { return }
                        startTypewriter(newText)
                    }
                } else {
                    startTypewriter(newText)
                }
            }
        }
        .task {
            startTypewriter(effectiveText(for: stateType))
            guard OverlayMotionRuntime.shouldAnimate else { return }
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(0.3))
                showCursor.toggle()
            }
        }
        .onChange(of: textOverride) { _, _ in
            let newText = effectiveText(for: stateType)
            if newText != targetText {
                startTypewriter(newText)
            }
        }
    }

    private func startTypewriter(_ text: String) {
        typewriterTask?.cancel()
        targetText = text
        displayedText = ""
        isTyping = true
        lastShownAt = .now
        typewriterTask = Task { @MainActor in
            for i in 0..<text.count {
                guard !Task.isCancelled else { return }
                let idx = text.index(text.startIndex, offsetBy: i)
                displayedText.append(text[idx])
                try? await Task.sleep(for: .seconds(charDelay))
            }
            isTyping = false
        }
    }

    private func effectiveText(for state: BobeStateType) -> String {
        if let override = textOverride, !override.isEmpty {
            return override
        }
        return labelText(for: state)
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
            ForEach(Array(chars.enumerated()), id: \.offset) { index, char in
                BubblingChar(
                    char: char,
                    color: theme.colors.primary,
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
        CGFloat.random(in: -45...45)
    }

    var body: some View {
        Text(char)
            .font(.system(size: 14, weight: .bold))
            .foregroundStyle(color)
            .frame(width: 16, height: 16)
            .offset(x: xPos, y: yOffset)
            .opacity(charOpacity)
            .task {
                guard OverlayMotionRuntime.shouldAnimate else {
                    charOpacity = 0
                    return
                }
                xPos = Self.randomX()
                let delay = Double(index) * 0.4
                try? await Task.sleep(for: .seconds(delay))
                guard !Task.isCancelled else { return }
                let duration = 2.5 + Double(index % 3) * 0.3
                while !Task.isCancelled {
                    xPos = Self.randomX()
                    yOffset = 30
                    charOpacity = 0
                    withAnimation(.easeIn(duration: duration * 0.2)) { charOpacity = 1 }
                    withAnimation(.easeOut(duration: duration)) { yOffset = -40 }
                    try? await Task.sleep(for: .seconds(duration * 0.8))
                    guard !Task.isCancelled else { return }
                    withAnimation(.easeOut(duration: duration * 0.2)) { charOpacity = 0 }
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
            ForEach(0..<barCount, id: \.self) { index in
                SpeakingBar(
                    angle: Double(index) * (360.0 / Double(barCount)),
                    delay: Double(index) * 0.08,
                    color: theme.colors.secondary
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
            .fill(color)
            .frame(width: 4, height: 12)
            .scaleEffect(y: scaleY)
            .offset(y: -52)
            .rotationEffect(.degrees(angle))
            .task {
                guard OverlayMotionRuntime.shouldAnimate else {
                    scaleY = 0.6
                    return
                }
                try? await Task.sleep(for: .seconds(delay))
                let frames: [CGFloat] = [0.3, 1.0, 0.5, 0.8, 0.3]
                var i = 0
                while !Task.isCancelled {
                    withAnimation(.easeInOut(duration: 0.12)) {
                        scaleY = frames[i % frames.count]
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
            .stroke(theme.colors.primary, lineWidth: 3)
            .frame(width: 124, height: 124)
            .scaleEffect(scale)
            .opacity(opacity)
            .onAppear {
                guard OverlayMotionRuntime.shouldAnimate else {
                    scale = 1.04
                    opacity = 0.85
                    return
                }
                withAnimation(.easeInOut(duration: 1.5).repeatForever(autoreverses: true)) {
                    scale = 1.08
                    opacity = 1.0
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
