import SwiftUI

// MARK: - Control Sizing

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
}

// MARK: - Design Tokens

enum BobeTextStyle {
    case windowTitle
    case sectionLabel
    case rowTitle
    case rowMeta
    case helper
    case body
    case badge

    var font: Font {
        switch self {
        case .windowTitle:
            .system(size: 28, weight: .semibold)
        case .sectionLabel:
            .system(size: 10, weight: .semibold)
        case .rowTitle:
            .system(size: 13, weight: .semibold)
        case .rowMeta:
            .system(size: 11)
        case .helper:
            .system(size: 11)
        case .body:
            .system(size: 12)
        case .badge:
            .system(size: 9, weight: .medium)
        }
    }
}

enum BobeMetrics {
    static let paneHorizontalPadding: CGFloat = 12
    static let paneTopPadding: CGFloat = 12
    static let listRowMinHeight: CGFloat = 54
    static let listRowCornerRadius: CGFloat = 10
}

// MARK: - Themed Button Styles

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
        let disabledOpacity = isEnabled ? 1.0 : 0.5
        let isPressed = configuration.isPressed

        configuration.label
            .font(.system(size: size.fontSize, weight: .semibold))
            .foregroundStyle(foregroundColor.opacity(disabledOpacity))
            .padding(.horizontal, size.horizontalPadding)
            .padding(.vertical, size.verticalPadding)
            .background(
                RoundedRectangle(cornerRadius: 8)
                    .fill(backgroundColor(isPressed: isPressed).opacity(disabledOpacity))
            )
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(borderColor(isPressed: isPressed).opacity(disabledOpacity), lineWidth: 1)
            )
            .opacity(isPressed ? 0.9 : 1)
            .animation(.easeOut(duration: 0.12), value: isPressed)
    }

    private var foregroundColor: Color {
        switch variant {
        case .primary:
            return theme.colors.background
        case .secondary:
            return theme.colors.text
        case .ghost:
            return theme.colors.textMuted
        case .destructive:
            return theme.colors.background
        }
    }

    private func backgroundColor(isPressed: Bool) -> Color {
        switch variant {
        case .primary:
            return isPressed
                ? theme.colors.primary.opacity(0.85)
                : hovered ? theme.colors.primary.opacity(0.94) : theme.colors.primary
        case .secondary:
            return isPressed
                ? theme.colors.border.opacity(0.8)
                : hovered ? theme.colors.border.opacity(0.5) : theme.colors.surface
        case .ghost:
            return isPressed
                ? theme.colors.border.opacity(0.5)
                : hovered ? theme.colors.surface : .clear
        case .destructive:
            return isPressed
                ? theme.colors.primary.opacity(0.9)
                : hovered ? theme.colors.primary.opacity(0.94) : theme.colors.primary
        }
    }

    private func borderColor(isPressed: Bool) -> Color {
        switch variant {
        case .primary:
            return theme.colors.primary.opacity(isPressed ? 0.95 : 1)
        case .secondary:
            return isPressed || hovered ? theme.colors.primary.opacity(0.65) : theme.colors.border
        case .ghost:
            return hovered ? theme.colors.border.opacity(0.7) : .clear
        case .destructive:
            return theme.colors.primary.opacity(isPressed ? 0.95 : 1)
        }
    }
}

private struct BobeButtonModifier: ViewModifier {
    let variant: BobeButtonVariant
    let size: BobeControlSize
    @State private var isHovered = false

    func body(content: Content) -> some View {
        content
            .buttonStyle(BobeButtonStyle(variant: variant, size: size, hovered: isHovered))
            .onHover { isHovered = $0 }
    }
}

extension View {
    func bobeTextStyle(_ style: BobeTextStyle) -> some View {
        font(style.font)
    }

    func bobeButton(
        _ variant: BobeButtonVariant = .secondary,
        size: BobeControlSize = .regular
    ) -> some View {
        modifier(BobeButtonModifier(variant: variant, size: size))
    }
}

// MARK: - Input Chrome

private struct BobeInputChromeModifier: ViewModifier {
    let focused: Bool
    let hovered: Bool
    @Environment(\.theme) private var theme

    func body(content: Content) -> some View {
        content
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(
                RoundedRectangle(cornerRadius: 8)
                    .fill(theme.colors.surface)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(
                        focused
                            ? theme.colors.primary
                            : hovered ? theme.colors.primary.opacity(0.45) : theme.colors.border,
                        lineWidth: focused ? 1.5 : 1
                    )
            )
            .shadow(
                color: focused ? theme.colors.primary.opacity(theme.isDark ? 0.2 : 0.12) : .clear,
                radius: focused ? 3 : 0
            )
    }
}

extension View {
    func bobeInputChrome(focused: Bool, hovered: Bool) -> some View {
        modifier(BobeInputChromeModifier(focused: focused, hovered: hovered))
    }
}

struct BobeTextField: View {
    let placeholder: String
    @Binding var text: String
    var width: CGFloat?
    var alignment: TextAlignment
    var onSubmit: (() -> Void)?

    init(
        placeholder: String,
        text: Binding<String>,
        width: CGFloat? = nil,
        alignment: TextAlignment = .leading,
        onSubmit: (() -> Void)? = nil
    ) {
        self.placeholder = placeholder
        self._text = text
        self.width = width
        self.alignment = alignment
        self.onSubmit = onSubmit
    }

    @Environment(\.theme) private var theme
    @FocusState private var isFocused: Bool
    @State private var isHovered = false

    var body: some View {
        TextField(
            "",
            text: $text,
            prompt: Text(placeholder)
                .foregroundStyle(theme.colors.textMuted)
        )
        .textFieldStyle(.plain)
        .font(.system(size: 13))
        .foregroundStyle(theme.colors.text)
        .multilineTextAlignment(alignment)
        .tint(theme.colors.primary)
        .focused($isFocused)
        .bobeInputChrome(focused: isFocused, hovered: isHovered)
        .onHover { isHovered = $0 }
        .onSubmit { onSubmit?() }
        .frame(width: width)
    }
}

struct BobeSecureField: View {
    let placeholder: String
    @Binding var text: String
    var width: CGFloat?
    var onSubmit: (() -> Void)?

    init(
        placeholder: String,
        text: Binding<String>,
        width: CGFloat? = nil,
        onSubmit: (() -> Void)? = nil
    ) {
        self.placeholder = placeholder
        self._text = text
        self.width = width
        self.onSubmit = onSubmit
    }

    @Environment(\.theme) private var theme
    @FocusState private var isFocused: Bool
    @State private var isHovered = false

    var body: some View {
        SecureField(
            "",
            text: $text,
            prompt: Text(placeholder)
                .foregroundStyle(theme.colors.textMuted)
        )
        .textFieldStyle(.plain)
        .font(.system(size: 13))
        .foregroundStyle(theme.colors.text)
        .tint(theme.colors.primary)
        .focused($isFocused)
        .bobeInputChrome(focused: isFocused, hovered: isHovered)
        .onHover { isHovered = $0 }
        .onSubmit { onSubmit?() }
        .frame(width: width)
    }
}

struct BobeMenuPicker<Option: Hashable>: View {
    @Binding var selection: Option
    let options: [Option]
    let label: (Option) -> String
    var width: CGFloat?
    var size: BobeControlSize

    init(
        selection: Binding<Option>,
        options: [Option],
        label: @escaping (Option) -> String,
        width: CGFloat? = nil,
        size: BobeControlSize = .regular
    ) {
        self._selection = selection
        self.options = options
        self.label = label
        self.width = width
        self.size = size
    }

    @Environment(\.theme) private var theme
    @State private var isHovered = false
    @State private var isOpen = false

    var body: some View {
        Button {
            withAnimation(.easeOut(duration: 0.12)) {
                isOpen.toggle()
            }
        } label: {
            HStack(spacing: 8) {
                Text(label(selection))
                    .font(.system(size: size.fontSize, weight: .medium))
                    .foregroundStyle(theme.colors.text)
                    .lineLimit(1)
                Spacer(minLength: 0)
                Image(systemName: isOpen ? "chevron.up" : "chevron.down")
                    .font(.system(size: max(9, size.fontSize - 2), weight: .semibold))
                    .foregroundStyle(theme.colors.textMuted)
            }
            .padding(.horizontal, max(8, size.horizontalPadding - 1))
            .padding(.vertical, max(5, size.verticalPadding))
            .background(
                RoundedRectangle(cornerRadius: 8)
                    .fill(theme.colors.surface)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(
                        isOpen ? theme.colors.primary : isHovered ? theme.colors.primary.opacity(0.55) : theme.colors.border,
                        lineWidth: 1
                    )
            )
        }
        .buttonStyle(.plain)
        .onHover { isHovered = $0 }
        .frame(width: width)
        .overlay(alignment: .topLeading) {
            if isOpen {
                dropdownPanel
                    .offset(y: controlHeight + 6)
                    .transition(.opacity.combined(with: .scale(scale: 0.96, anchor: .top)))
            }
        }
        .onChange(of: selection) { _, _ in
            isOpen = false
        }
        .zIndex(isOpen ? 50 : 0)
    }

    private var controlHeight: CGFloat {
        max(30, size.fontSize + (size.verticalPadding * 2) + 8)
    }

    private var dropdownPanel: some View {
        ScrollView {
            LazyVStack(spacing: 2) {
                ForEach(options, id: \.self) { option in
                    BobeDropdownOptionRow(
                        text: label(option),
                        isSelected: option == selection
                    ) {
                        selection = option
                    }
                }
            }
            .padding(4)
        }
        .frame(maxHeight: min(CGFloat(options.count) * 30 + 8, 220))
        .frame(width: width ?? 220, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 10)
                .fill(theme.colors.surface)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 10)
                .stroke(theme.colors.border, lineWidth: 1)
        )
        .shadow(color: .black.opacity(0.12), radius: 6, y: 4)
    }
}

private struct BobeDropdownOptionRow: View {
    let text: String
    let isSelected: Bool
    let action: () -> Void

    @Environment(\.theme) private var theme
    @State private var isHovered = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                Text(text)
                    .font(.system(size: 12, weight: isSelected ? .semibold : .regular))
                    .foregroundStyle(theme.colors.text)
                    .lineLimit(1)
                Spacer(minLength: 0)
                if isSelected {
                    Image(systemName: "checkmark")
                        .font(.system(size: 10, weight: .bold))
                        .foregroundStyle(theme.colors.primary)
                }
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 6)
            .background(
                RoundedRectangle(cornerRadius: 7)
                    .fill(
                        isSelected
                            ? theme.colors.primary.opacity(theme.isDark ? 0.26 : 0.16)
                            : isHovered ? theme.colors.background : .clear
                    )
            )
        }
        .buttonStyle(.plain)
        .onHover { isHovered = $0 }
    }
}

// MARK: - Themed Progress Indicators

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
                .stroke(theme.colors.border.opacity(0.6), lineWidth: lineWidth)
            Circle()
                .trim(from: 0.12, to: 0.82)
                .stroke(
                    color ?? theme.colors.primary,
                    style: StrokeStyle(lineWidth: lineWidth, lineCap: .round)
                )
                .rotationEffect(.degrees(spinning ? 360 : 0))
                .animation(.linear(duration: 0.85).repeatForever(autoreverses: false), value: spinning)
        }
        .frame(width: size, height: size)
        .onAppear { spinning = true }
        .onDisappear { spinning = false }
    }
}

struct BobeLinearProgressBar: View {
    let progress: Double
    var height: CGFloat = 7

    @Environment(\.theme) private var theme

    private var clampedProgress: Double {
        min(max(progress, 0), 1)
    }

    var body: some View {
        GeometryReader { geo in
            ZStack(alignment: .leading) {
                Capsule()
                    .fill(theme.colors.border.opacity(0.55))
                Capsule()
                    .fill(
                        LinearGradient(
                            colors: [theme.colors.primary, theme.colors.secondary],
                            startPoint: .leading,
                            endPoint: .trailing
                        )
                    )
                    .frame(width: max(4, geo.size.width * clampedProgress))
            }
        }
        .frame(height: height)
    }
}

// MARK: - Selectable Row

struct BobeSelectableRow<Content: View>: View {
    let isSelected: Bool
    let action: () -> Void
    @ViewBuilder let content: Content

    @Environment(\.theme) private var theme
    @State private var isHovered = false

    init(
        isSelected: Bool,
        action: @escaping () -> Void,
        @ViewBuilder content: () -> Content
    ) {
        self.isSelected = isSelected
        self.action = action
        self.content = content()
    }

    var body: some View {
        Button(action: action) {
            HStack(spacing: 10) {
                content
            }
            .foregroundStyle(theme.colors.text)
            .frame(maxWidth: .infinity, minHeight: BobeMetrics.listRowMinHeight, alignment: .leading)
            .padding(.horizontal, 8)
            .padding(.vertical, 6)
            .background(
                RoundedRectangle(cornerRadius: BobeMetrics.listRowCornerRadius)
                    .fill(backgroundColor)
            )
            .overlay(
                RoundedRectangle(cornerRadius: BobeMetrics.listRowCornerRadius)
                    .stroke(borderColor, lineWidth: borderLineWidth)
            )
        }
        .buttonStyle(BobePressFeedbackStyle())
        .contentShape(Rectangle())
        .onHover { isHovered = $0 }
    }

    private var backgroundColor: Color {
        if isSelected {
            return theme.colors.primary.opacity(theme.isDark ? 0.26 : 0.15)
        }
        if isHovered {
            return theme.colors.surface
        }
        return .clear
    }

    private var borderColor: Color {
        if isSelected {
            return theme.colors.primary.opacity(0.55)
        }
        if isHovered {
            return theme.colors.border
        }
        return .clear
    }

    private var borderLineWidth: CGFloat {
        isSelected || isHovered ? 1 : 0
    }
}

// MARK: - Shared interactive rows

struct BobePressFeedbackStyle: ButtonStyle {
    var pressedOpacity: Double = 0.92
    var pressedScale: CGFloat = 0.995

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .opacity(configuration.isPressed ? pressedOpacity : 1)
            .scaleEffect(configuration.isPressed ? pressedScale : 1)
            .animation(.easeOut(duration: 0.1), value: configuration.isPressed)
    }
}

struct BobeSidebarItem: View {
    let icon: String
    let title: String
    let isSelected: Bool
    let action: () -> Void

    @Environment(\.theme) private var theme
    @State private var isHovered = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: 10) {
                Image(systemName: icon)
                    .font(.system(size: 14))
                    .foregroundStyle(isSelected ? theme.colors.primary : theme.colors.textMuted)
                    .frame(width: 18)

                Text(title)
                    .bobeTextStyle(.rowTitle)
                    .fontWeight(isSelected ? .semibold : .regular)
                    .foregroundStyle(isSelected ? theme.colors.primary : theme.colors.text)

                Spacer()
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 12)
            .padding(.vertical, 10)
            .background(
                RoundedRectangle(cornerRadius: 8)
                    .fill(backgroundColor)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(borderColor, lineWidth: borderLineWidth)
            )
            .overlay(alignment: .leading) {
                if isSelected {
                    RoundedRectangle(cornerRadius: 2)
                        .fill(theme.colors.primary)
                        .frame(width: 3)
                        .padding(.vertical, 7)
                }
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(BobePressFeedbackStyle())
        .onHover { isHovered = $0 }
    }

    private var backgroundColor: Color {
        if isSelected {
            return theme.colors.primary.opacity(theme.isDark ? 0.24 : 0.14)
        }
        if isHovered {
            return theme.colors.surface
        }
        return .clear
    }

    private var borderColor: Color {
        if isSelected {
            return theme.colors.primary.opacity(0.45)
        }
        if isHovered {
            return theme.colors.border
        }
        return .clear
    }

    private var borderLineWidth: CGFloat {
        isSelected || isHovered ? 1 : 0
    }
}

// MARK: - Previews

#Preview("Button Variants") {
    VStack(spacing: 12) {
        HStack(spacing: 8) {
            Button("Primary") {}.bobeButton(.primary, size: .regular)
            Button("Secondary") {}.bobeButton(.secondary, size: .regular)
            Button("Ghost") {}.bobeButton(.ghost, size: .regular)
            Button("Destructive") {}.bobeButton(.destructive, size: .regular)
        }
        HStack(spacing: 8) {
            Button("Small") {}.bobeButton(.primary, size: .small)
            Button("Mini") {}.bobeButton(.primary, size: .mini)
        }
    }
    .environment(\.theme, allThemes[0])
    .padding()
}

#Preview("BobeTextField") {
    @Previewable @State var text = ""
    VStack(spacing: 12) {
        BobeTextField(placeholder: "Enter something...", text: $text)
        BobeTextField(placeholder: "Fixed width", text: $text, width: 200)
    }
    .environment(\.theme, allThemes[0])
    .padding()
    .frame(width: 400)
}

#Preview("BobeSecureField") {
    @Previewable @State var text = ""
    BobeSecureField(placeholder: "sk-...", text: $text, width: 300)
        .environment(\.theme, allThemes[0])
        .padding()
}

#Preview("BobeMenuPicker") {
    @Previewable @State var selection = "Option A"
    BobeMenuPicker(
        selection: $selection,
        options: ["Option A", "Option B", "Option C"],
        label: { $0 },
        width: 200
    )
    .environment(\.theme, allThemes[0])
    .padding()
    .frame(height: 300)
}

#Preview("BobeSpinner + Progress") {
    VStack(spacing: 16) {
        HStack(spacing: 16) {
            BobeSpinner(size: 14)
            BobeSpinner(size: 20)
            BobeSpinner(size: 28)
        }
        BobeLinearProgressBar(progress: 0.65)
            .frame(width: 300)
        BobeLinearProgressBar(progress: 0.2)
            .frame(width: 300)
    }
    .environment(\.theme, allThemes[0])
    .padding()
}

#Preview("BobeSidebarItem") {
    VStack(spacing: 4) {
        BobeSidebarItem(icon: "paintpalette.fill", title: "Appearance", isSelected: true, action: {})
        BobeSidebarItem(icon: "gearshape.fill", title: "Behavior", isSelected: false, action: {})
        BobeSidebarItem(icon: "wrench.fill", title: "Tools", isSelected: false, action: {})
    }
    .environment(\.theme, allThemes[0])
    .frame(width: 200)
    .padding()
}

#Preview("BobeSelectableRow") {
    VStack(spacing: 6) {
        BobeSelectableRow(isSelected: true, action: {}) {
            Text("Selected item")
        }
        BobeSelectableRow(isSelected: false, action: {}) {
            Text("Unselected item")
        }
    }
    .environment(\.theme, allThemes[0])
    .frame(width: 300)
    .padding()
}
