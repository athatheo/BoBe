import SwiftUI

// MARK: - Status Label (pill at top of avatar, typewriter + blinking cursor)

struct StatusLabel: View {
    let stateType: BobeStateType

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
            startTypewriter(labelText(for: stateType))
            // Cursor blink loop — auto-cancelled when view disappears
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(0.3))
                showCursor.toggle()
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

    private func labelText(for state: BobeStateType) -> String {
        switch state {
        case .loading: "Loading"
        case .idle: ""
        case .capturing: "Capturing"
        case .thinking: "Thinking"
        case .speaking: "Speaking"
        case .wantsToSpeak: "Hey"
        case .error: "Offline"
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
            .task {
                let delay = Double(index) * 0.4
                try? await Task.sleep(for: .seconds(delay))
                guard !Task.isCancelled else { return }
                let duration = 2.5 + Double(index % 3) * 0.3
                while !Task.isCancelled {
                    yOffset = 30
                    opacity = 0
                    withAnimation(.easeOut(duration: duration)) { yOffset = -40 }
                    withAnimation(.easeIn(duration: 0.3)) { opacity = 1 }
                    try? await Task.sleep(for: .seconds(duration * 0.7))
                    guard !Task.isCancelled else { return }
                    withAnimation(.easeOut(duration: duration * 0.3)) { opacity = 0 }
                    try? await Task.sleep(for: .seconds(duration * 0.3))
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
