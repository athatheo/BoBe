import SwiftUI

// MARK: - Status Label (pill at top of avatar, typewriter + blinking cursor)

struct StatusLabel: View {
    let stateType: BobeStateType

    @State private var displayedText = ""
    @State private var targetText = ""
    @State private var isTyping = false
    @State private var showCursor = true
    @State private var lastShownAt: Date = .distantPast
    @Environment(\.theme) private var theme

    private let charDelay: TimeInterval = 0.04
    private let minDisplayTime: TimeInterval = 2.0

    var body: some View {
        Group {
            if !targetText.isEmpty || isTyping {
                HStack(spacing: 0) {
                    Text(displayedText)
                        .font(.system(size: 11, weight: .bold))
                        .tracking(0.5)
                        .foregroundStyle(theme.colors.primary)

                    // Blinking cursor
                    if isTyping || !displayedText.isEmpty {
                        Text("|")
                            .font(.system(size: 11, weight: .bold))
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
        .animation(.easeInOut(duration: 0.2), value: displayedText)
        .onChange(of: stateType) { _, newState in
            let newText = labelText(for: newState)
            if newText != targetText {
                let elapsed = Date().timeIntervalSince(lastShownAt)
                if elapsed < minDisplayTime && !targetText.isEmpty {
                    Task { @MainActor in
                        try? await Task.sleep(for: .seconds(minDisplayTime - elapsed))
                        startTypewriter(newText)
                    }
                } else {
                    startTypewriter(newText)
                }
            }
        }
        .onAppear {
            startTypewriter(labelText(for: stateType))
            startCursorBlink()
        }
    }

    private func startTypewriter(_ text: String) {
        targetText = text
        displayedText = ""
        isTyping = true
        lastShownAt = .now
        typeNextChar(of: text, index: 0)
    }

    private func typeNextChar(of text: String, index: Int) {
        guard index < text.count else {
            isTyping = false
            return
        }
        let idx = text.index(text.startIndex, offsetBy: index)
        displayedText.append(text[idx])
        Task { @MainActor in
            try? await Task.sleep(for: .seconds(charDelay))
            typeNextChar(of: text, index: index + 1)
        }
    }

    private func startCursorBlink() {
        Task { @MainActor in
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(0.3))
                showCursor.toggle()
            }
        }
    }

    private func labelText(for state: BobeStateType) -> String {
        switch state {
        case .loading: "connecting..."
        case .idle: ""
        case .capturing: "looking..."
        case .thinking: "thinking..."
        case .speaking: "speaking"
        case .wantsToSpeak: "has something to say"
        case .error: "error"
        }
    }
}

// MARK: - Thinking Numbers Ring (terracotta chars in the ring gap)

struct ThinkingNumbersRing: View {
    private let chars: [String] = ["1", "+", "0", "=", "π", "7", "%", "∑", "×", "2", "/", "9"]
    @Environment(\.theme) private var theme

    var body: some View {
        ZStack {
            ForEach(Array(chars.enumerated()), id: \.offset) { index, char in
                BubblingChar(
                    char: char,
                    color: theme.colors.primary,
                    index: index,
                    total: chars.count
                )
            }
        }
        .frame(width: 116, height: 116)
    }
}

private struct BubblingChar: View {
    let char: String
    let color: Color
    let index: Int
    let total: Int

    @State private var yOffset: CGFloat = 30
    @State private var opacity: Double = 0

    // Distribute chars around the ring gap area
    private var xPosition: CGFloat {
        let angle = (Double(index) / Double(total)) * 2 * .pi - .pi / 2
        return cos(angle) * 45
    }

    var body: some View {
        Text(char)
            .font(.system(size: 14, weight: .bold))
            .foregroundStyle(color)
            .frame(width: 16, height: 16)
            .offset(x: xPosition, y: yOffset)
            .opacity(opacity)
            .onAppear {
                let delay = Double(index) * 0.4
                Task { @MainActor in
                    try? await Task.sleep(for: .seconds(delay))
                    animate()
                }
            }
    }

    private func animate() {
        let duration = 2.5 + Double(index % 3) * 0.3
        withAnimation(.easeOut(duration: duration).repeatForever(autoreverses: false)) {
            yOffset = -40
        }
        // Fade in then out
        withAnimation(.easeIn(duration: 0.3)) {
            opacity = 1
        }
        Task { @MainActor in
            try? await Task.sleep(for: .seconds(duration * 0.7))
            withAnimation(.easeOut(duration: duration * 0.3)) {
                opacity = 0
            }
        }
    }
}

// MARK: - Speaking Wave Ring (16 olive bars around avatar)

struct SpeakingWaveRing: View {
    private let barCount = 16
    @Environment(\.theme) private var theme

    var body: some View {
        ZStack {
            ForEach(0..<barCount, id: \.self) { index in
                SpeakingBar(
                    angle: Double(index) * (360.0 / Double(barCount)),
                    delay: Double(index) * 0.05,
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
            .offset(y: -52) // Position in the ring gap
            .rotationEffect(.degrees(angle))
            .onAppear {
                withAnimation(
                    .easeInOut(duration: 0.6)
                    .repeatForever(autoreverses: true)
                    .delay(delay)
                ) {
                    scaleY = 1.0
                }
            }
    }
}

// MARK: - Attention Pulse (terracotta ring pulsing)

struct AttentionPulse: View {
    @State private var scale: CGFloat = 1.0
    @State private var opacity: Double = 0.7
    @Environment(\.theme) private var theme

    var body: some View {
        Circle()
            .stroke(theme.colors.primary, lineWidth: 3)
            .frame(width: 124, height: 124) // 4px larger than avatar card on each side
            .scaleEffect(scale)
            .opacity(opacity)
            .onAppear {
                withAnimation(.easeInOut(duration: 1.5).repeatForever(autoreverses: true)) {
                    scale = 1.08
                    opacity = 1.0
                }
            }
    }
}
