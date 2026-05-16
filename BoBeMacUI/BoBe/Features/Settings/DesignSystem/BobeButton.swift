import SwiftUI

enum BobeButtonVariant {
    case primary
    case secondary
    case ghost
    case destructive
}

struct BobeButtonStyle: ButtonStyle {
    let variant: BobeButtonVariant
    let size: BobeControlSize
    let hovered: Bool

    @Environment(\.theme) private var theme
    @Environment(\.isEnabled) private var isEnabled

    init(
        variant: BobeButtonVariant = .secondary,
        size: BobeControlSize = .regular,
        hovered: Bool = false
    ) {
        self.variant = variant
        self.size = size
        self.hovered = hovered
    }

    func makeBody(configuration: Configuration) -> some View {
        let disabledOpacity = self.isEnabled ? 1.0 : 0.5
        let isPressed = configuration.isPressed

        configuration.label
            .font(.system(size: self.size.fontSize, weight: .semibold))
            .foregroundStyle(self.foregroundColor.opacity(disabledOpacity))
            .padding(.horizontal, self.size.horizontalPadding)
            .padding(.vertical, self.size.verticalPadding)
            .background(
                RoundedRectangle(cornerRadius: 8)
                    .fill(self.backgroundColor(isPressed: isPressed).opacity(disabledOpacity))
            )
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(self.borderColor(isPressed: isPressed).opacity(disabledOpacity), lineWidth: 1)
            )
            .opacity(isPressed ? 0.9 : 1)
            .animation(OverlayMotionRuntime.reduceMotion ? nil : .easeOut(duration: 0.12), value: isPressed)
    }

    private var foregroundColor: Color {
        switch self.variant {
        case .primary:
            self.theme.colors.background
        case .secondary:
            self.theme.colors.text
        case .ghost:
            self.theme.colors.textMuted
        case .destructive:
            self.theme.colors.background
        }
    }

    private func backgroundColor(isPressed: Bool) -> Color {
        switch self.variant {
        case .primary:
            isPressed
                ? self.theme.colors.primary.opacity(0.85)
                : self.hovered ? self.theme.colors.primary.opacity(0.94) : self.theme.colors.primary
        case .secondary:
            isPressed
                ? self.theme.colors.border.opacity(0.8)
                : self.hovered ? self.theme.colors.border.opacity(0.5) : self.theme.colors.surface
        case .ghost:
            isPressed
                ? self.theme.colors.border.opacity(0.5)
                : self.hovered ? self.theme.colors.surface : .clear
        case .destructive:
            isPressed
                ? self.theme.colors.primary.opacity(0.9)
                : self.hovered ? self.theme.colors.primary.opacity(0.94) : self.theme.colors.primary
        }
    }

    private func borderColor(isPressed: Bool) -> Color {
        switch self.variant {
        case .primary:
            self.theme.colors.primary.opacity(isPressed ? 0.95 : 1)
        case .secondary:
            isPressed || self.hovered ? self.theme.colors.primary.opacity(0.65) : self.theme.colors.border
        case .ghost:
            self.hovered ? self.theme.colors.border.opacity(0.7) : .clear
        case .destructive:
            self.theme.colors.primary.opacity(isPressed ? 0.95 : 1)
        }
    }
}

private struct BobeButtonModifier: ViewModifier {
    let variant: BobeButtonVariant
    let size: BobeControlSize
    @State private var isHovered = false

    func body(content: Content) -> some View {
        content
            .buttonStyle(BobeButtonStyle(variant: self.variant, size: self.size, hovered: self.isHovered))
            .onHover { self.isHovered = $0 }
    }
}

extension View {
    func bobeButton(
        _ variant: BobeButtonVariant = .secondary,
        size: BobeControlSize = .regular
    ) -> some View {
        modifier(BobeButtonModifier(variant: variant, size: size))
    }
}
