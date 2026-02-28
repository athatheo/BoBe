import SwiftUI

/// Indicator bubble showing thinking/analyzing state and tool execution progress.
/// CSS: positioned LEFT of avatar (right: calc(100% + 12px), top: 50%, translateY(-50%))
/// This view is placed inside the avatar-with-indicator container, offset to the left.
struct IndicatorBubble: View {
    let indicator: IndicatorType?
    let toolExecutions: [ToolExecution]

    @State private var displayIndicator: IndicatorType?
    @State private var showTime: Date?
    @State private var isExpanded = false
    @State private var delayTask: Task<Void, Never>?
    @Environment(\.theme) private var theme

    var body: some View {
        Group {
            if displayIndicator != nil || !activeTools.isEmpty {
                bubbleContent
                    .transition(
                        .asymmetric(
                            insertion: .offset(x: 10).combined(with: .opacity).combined(with: .scale(scale: 0.9)),
                            removal: .offset(x: 10).combined(with: .opacity).combined(with: .scale(scale: 0.9))
                        )
                    )
            }
        }
        .animation(OverlayMotionRuntime.animation(for: .indicatorTransition), value: displayIndicator != nil)
        .animation(OverlayMotionRuntime.animation(for: .indicatorTransition), value: activeTools.count)
        .onChange(of: indicator) { _, newValue in
            handleIndicatorChange(newValue)
        }
    }

    @ViewBuilder
    private var bubbleContent: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Main content
            VStack(alignment: .leading, spacing: 4) {
                if !activeTools.isEmpty {
                    toolContent
                } else if let ind = displayIndicator {
                    indicatorContent(ind)
                }
            }

            // Expandable tool history
            if isExpanded && !completedTools.isEmpty {
                VStack(alignment: .leading, spacing: 4) {
                    Divider()
                        .background(theme.colors.border)
                    toolHistory
                }
                .padding(.top, 6)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
        .background(
            RoundedRectangle(cornerRadius: 16)
                .fill(theme.colors.background)
                .overlay(
                    RoundedRectangle(cornerRadius: 16)
                        .stroke(theme.colors.border, lineWidth: 1.5)
                )
                .shadow(color: Color.black.opacity(0.08), radius: 4, y: 2)
        )
        .frame(maxWidth: 220, alignment: .leading)
    }

    private func indicatorContent(_ indicator: IndicatorType) -> some View {
        HStack(spacing: 6) {
            Image(systemName: "sparkle")
                .font(.system(size: 10))
                .foregroundStyle(theme.colors.primary)

            Text(indicator == .toolCalling ? "using tools" : "thinking")
                .font(.system(size: 11, weight: .medium))
                .foregroundStyle(theme.colors.textMuted)

            AnimatedDots(color: theme.colors.textMuted)
        }
    }

    private var toolContent: some View {
        VStack(alignment: .leading, spacing: 4) {
            // Current tool row
            HStack(spacing: 6) {
                RotatingWrench(color: theme.colors.primary)

                Text(activeTools.first?.toolName ?? "")
                    .font(.system(size: 11, weight: .medium))
                    .foregroundStyle(theme.colors.textMuted)
                    .lineLimit(1)

                AnimatedDots(color: theme.colors.textMuted)

                // Expand button with count badge
                if !completedTools.isEmpty {
                    Button {
                        withAnimation(OverlayMotionRuntime.animation(for: .indicatorTransition)) {
                            isExpanded.toggle()
                        }
                    } label: {
                        Image(systemName: isExpanded ? "chevron.up" : "chevron.down")
                            .font(.system(size: 8))
                            .foregroundStyle(theme.colors.textMuted)
                            .padding(2)
                    }
                    .buttonStyle(.plain)
                }
            }
        }
    }

    private var toolHistory: some View {
        VStack(alignment: .leading, spacing: 4) {
            ForEach(completedTools.suffix(5)) { tool in
                HStack(spacing: 4) {
                    Image(systemName: tool.status == .success ? "checkmark" : "xmark")
                        .font(.system(size: 8, weight: .bold))
                        .foregroundStyle(tool.status == .success ? theme.colors.secondary : theme.colors.primary)

                    Text(tool.toolName)
                        .font(.system(size: 10))
                        .foregroundStyle(theme.colors.textMuted)
                        .lineLimit(1)

                    if let duration = tool.durationMs {
                        Spacer()
                        Text("\(duration)ms")
                            .font(.system(size: 9))
                            .foregroundStyle(theme.colors.textMuted.opacity(0.6))
                    }
                }
            }
        }
    }

    private var activeTools: [ToolExecution] {
        toolExecutions.filter { $0.status == .running }
    }

    private var completedTools: [ToolExecution] {
        toolExecutions.filter { $0.status != .running }
    }

    private func handleIndicatorChange(_ newIndicator: IndicatorType?) {
        delayTask?.cancel()
        if let newIndicator, newIndicator == .thinking || newIndicator == .toolCalling {
            delayTask = Task { @MainActor in
                try? await Task.sleep(for: .seconds(IndicatorTiming.delayBeforeShow))
                guard !Task.isCancelled, indicator == newIndicator else { return }
                displayIndicator = newIndicator
                showTime = .now
            }
        } else if newIndicator == nil {
            if let showTime {
                let elapsed = Date().timeIntervalSince(showTime)
                if elapsed < IndicatorTiming.minDisplayTime {
                    delayTask = Task { @MainActor in
                        try? await Task.sleep(for: .seconds(IndicatorTiming.minDisplayTime - elapsed))
                        guard !Task.isCancelled else { return }
                        displayIndicator = nil
                        self.showTime = nil
                    }
                    return
                }
            }
            displayIndicator = nil
            showTime = nil
        }
    }
}

/// Rotating wrench icon for tool execution
private struct RotatingWrench: View {
    let color: Color
    @State private var rotation: Double = 0

    var body: some View {
        Image(systemName: "wrench.fill")
            .font(.system(size: 10))
            .foregroundStyle(color)
            .rotationEffect(.degrees(rotation))
            .onAppear {
                withAnimation(.linear(duration: 2).repeatForever(autoreverses: false)) {
                    rotation = 360
                }
            }
    }
}

/// Animated three dots with staggered opacity (matches indicator-dot CSS)
struct AnimatedDots: View {
    var color: Color = .secondary

    @State private var opacities: [Double] = [1, 0.3, 0.3]

    var body: some View {
        HStack(spacing: 1) {
            ForEach(0..<3, id: \.self) { index in
                Text(".")
                    .font(.system(size: 11, weight: .bold))
                    .foregroundStyle(color)
                    .opacity(opacities[index])
            }
        }
        .frame(width: 16, alignment: .leading)
        .task {
            var step = 0
            while !Task.isCancelled {
                try? await Task.sleep(for: .milliseconds(400))
                withAnimation(.easeInOut(duration: 0.2)) {
                    opacities = [0.3, 0.3, 0.3]
                    opacities[step % 3] = 1
                }
                step += 1
            }
        }
    }
}

// MARK: - Previews

#Preview("Thinking Indicator") {
    IndicatorBubble(indicator: .thinking, toolExecutions: [])
        .environment(\.theme, allThemes[0])
        .padding()
        .background(Color.gray.opacity(0.1))
}

#Preview("Tool Calling") {
    IndicatorBubble(
        indicator: .toolCalling,
        toolExecutions: [
            ToolExecution(toolName: "search_files", toolCallId: "1", status: .running, startedAt: .now),
            ToolExecution(toolName: "fetch_url", toolCallId: "2", status: .success, durationMs: 230, startedAt: .now, completedAt: .now),
        ]
    )
    .environment(\.theme, allThemes[0])
    .padding()
    .background(Color.gray.opacity(0.1))
}

#Preview("Animated Dots") {
    AnimatedDots(color: .secondary)
        .padding()
}
