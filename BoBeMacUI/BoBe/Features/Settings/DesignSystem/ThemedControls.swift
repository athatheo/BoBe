import SwiftUI

enum BobeControlSize {
    case mini
    case small
    case regular

    var fontSize: CGFloat {
        switch self {
        case .mini: 10
        case .small: 11
        case .regular: 13
        }
    }

    var horizontalPadding: CGFloat {
        switch self {
        case .mini: 7
        case .small: 9
        case .regular: 12
        }
    }

    var verticalPadding: CGFloat {
        switch self {
        case .mini: 4
        case .small: 5
        case .regular: 7
        }
    }

    var controlSize: ControlSize {
        switch self {
        case .mini: .mini
        case .small: .small
        case .regular: .regular
        }
    }
}

enum BobeMetrics {
    static let paneHorizontalPadding: CGFloat = 12
    static let paneTopPadding: CGFloat = 12
    static let listRowMinHeight: CGFloat = 54
    static let listRowCornerRadius: CGFloat = 10
}

extension View {
    func bobeTextStyle(_ style: BobeTextStyle) -> some View {
        font(style.font)
    }
}

struct BobeSpinner: View {
    var size: CGFloat = 14
    var lineWidth: CGFloat = 2
    var color: Color?

    init(size: CGFloat = 14, lineWidth: CGFloat = 2, color: Color? = nil) {
        self.size = size
        self.lineWidth = lineWidth
        self.color = color
    }

    @Environment(\.theme) private var theme
    @State private var spinning = false

    var body: some View {
        ZStack {
            Circle()
                .stroke(self.theme.colors.border.opacity(0.6), lineWidth: self.lineWidth)
            Circle()
                .trim(from: 0.12, to: 0.82)
                .stroke(
                    self.color ?? self.theme.colors.primary,
                    style: StrokeStyle(lineWidth: self.lineWidth, lineCap: .round)
                )
                .rotationEffect(.degrees(self.spinning ? 360 : 0))
                .animation(OverlayMotionRuntime.reduceMotion ? nil : .linear(duration: 0.85).repeatForever(autoreverses: false), value: self.spinning)
        }
        .frame(width: self.size, height: self.size)
        .onAppear { self.spinning = true }
        .onDisappear { self.spinning = false }
    }
}

struct BobeLinearProgressBar: View {
    let progress: Double
    var height: CGFloat = 7
    /// Optional solid fill. When provided, replaces the default left-to-right
    /// gradient — use this for value-coded gauges (e.g. memory usage) where a
    /// positional gradient would imply meaning the chart doesn't have.
    var tint: Color?

    @Environment(\.theme) private var theme

    private var clampedProgress: Double {
        min(max(self.progress, 0), 1)
    }

    var body: some View {
        GeometryReader { geo in
            ZStack(alignment: .leading) {
                Capsule()
                    .fill(self.theme.colors.textMuted.opacity(0.18))
                Capsule()
                    .fill(self.fillStyle)
                    .frame(width: max(4, geo.size.width * self.clampedProgress))
            }
        }
        .frame(height: self.height)
        .animation(.easeInOut(duration: 0.25), value: self.clampedProgress)
    }

    private var fillStyle: AnyShapeStyle {
        if let tint = self.tint {
            AnyShapeStyle(tint)
        } else {
            AnyShapeStyle(
                LinearGradient(
                    colors: [self.theme.colors.primary, self.theme.colors.secondary],
                    startPoint: .leading,
                    endPoint: .trailing
                )
            )
        }
    }
}

struct BobeSelectableRow<Content: View>: View {
    let isSelected: Bool
    @ViewBuilder let content: Content

    @Environment(\.theme) private var theme
    @State private var isHovered = false

    init(
        isSelected: Bool,
        @ViewBuilder content: () -> Content
    ) {
        self.isSelected = isSelected
        self.content = content()
    }

    var body: some View {
        HStack(spacing: 10) {
            self.content
        }
        .foregroundStyle(self.theme.colors.text)
        .frame(maxWidth: .infinity, minHeight: BobeMetrics.listRowMinHeight, alignment: .leading)
        .padding(.horizontal, 8)
        .padding(.vertical, 6)
        .background(
            RoundedRectangle(cornerRadius: BobeMetrics.listRowCornerRadius)
                .fill(self.backgroundColor)
        )
        .overlay(
            RoundedRectangle(cornerRadius: BobeMetrics.listRowCornerRadius)
                .stroke(self.borderColor, lineWidth: self.borderLineWidth)
        )
        .contentShape(Rectangle())
        .accessibilityElement(children: .contain)
        .accessibilityAddTraits(self.isSelected ? .isSelected : [])
        .onHover { self.isHovered = $0 }
    }

    private var backgroundColor: Color {
        if self.isSelected {
            return self.theme.colors.primary.opacity(self.theme.isDark ? 0.26 : 0.15)
        }
        if self.isHovered {
            return self.theme.colors.surface
        }
        return .clear
    }

    private var borderColor: Color {
        if self.isSelected {
            return self.theme.colors.primary.opacity(0.55)
        }
        if self.isHovered {
            return self.theme.colors.border
        }
        return .clear
    }

    private var borderLineWidth: CGFloat {
        self.isSelected || self.isHovered ? 1 : 0
    }
}
