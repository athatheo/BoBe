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
            .animation(.easeOut(duration: 0.12), value: isPressed)
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
                    .fill(self.theme.colors.surface)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(
                        self.focused
                            ? self.theme.colors.primary
                            : self.hovered ? self.theme.colors.primary.opacity(0.45) : self.theme.colors.border,
                        lineWidth: self.focused ? 1.5 : 1
                    )
            )
            .shadow(
                color: self.focused ? self.theme.colors.primary.opacity(self.theme.isDark ? 0.2 : 0.12) : .clear,
                radius: self.focused ? 3 : 0
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
        _text = text
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
            text: self.$text,
            prompt: Text(self.placeholder)
                .foregroundStyle(self.theme.colors.textMuted)
        )
        .textFieldStyle(.plain)
        .font(.system(size: 13))
        .foregroundStyle(self.theme.colors.text)
        .multilineTextAlignment(self.alignment)
        .tint(self.theme.colors.primary)
        .focused(self.$isFocused)
        .bobeInputChrome(focused: self.isFocused, hovered: self.isHovered)
        .onHover { self.isHovered = $0 }
        .onSubmit { self.onSubmit?() }
        .frame(width: self.width)
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
        _text = text
        self.width = width
        self.onSubmit = onSubmit
    }

    @Environment(\.theme) private var theme
    @FocusState private var isFocused: Bool
    @State private var isHovered = false

    var body: some View {
        SecureField(
            "",
            text: self.$text,
            prompt: Text(self.placeholder)
                .foregroundStyle(self.theme.colors.textMuted)
        )
        .textFieldStyle(.plain)
        .font(.system(size: 13))
        .foregroundStyle(self.theme.colors.text)
        .tint(self.theme.colors.primary)
        .focused(self.$isFocused)
        .bobeInputChrome(focused: self.isFocused, hovered: self.isHovered)
        .onHover { self.isHovered = $0 }
        .onSubmit { self.onSubmit?() }
        .frame(width: self.width)
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
        _selection = selection
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
                self.isOpen.toggle()
            }
        } label: {
            HStack(spacing: 8) {
                Text(self.label(self.selection))
                    .font(.system(size: self.size.fontSize, weight: .medium))
                    .foregroundStyle(self.theme.colors.text)
                    .lineLimit(1)
                Spacer(minLength: 0)
                Image(systemName: self.isOpen ? "chevron.up" : "chevron.down")
                    .font(.system(size: max(9, self.size.fontSize - 2), weight: .semibold))
                    .foregroundStyle(self.theme.colors.textMuted)
            }
            .padding(.horizontal, max(8, self.size.horizontalPadding - 1))
            .padding(.vertical, max(5, self.size.verticalPadding))
            .background(
                RoundedRectangle(cornerRadius: 8)
                    .fill(self.theme.colors.surface)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(
                        self.isOpen ? self.theme.colors.primary : self.isHovered ? self.theme.colors.primary.opacity(0.55) : self.theme.colors.border,
                        lineWidth: 1
                    )
            )
        }
        .buttonStyle(.plain)
        .onHover { self.isHovered = $0 }
        .frame(width: self.width)
        .overlay(alignment: .topLeading) {
            if self.isOpen {
                self.dropdownPanel
                    .offset(y: self.controlHeight + 6)
                    .transition(.opacity.combined(with: .scale(scale: 0.96, anchor: .top)))
            }
        }
        .onChange(of: self.selection) { _, _ in
            self.isOpen = false
        }
        .zIndex(self.isOpen ? 50 : 0)
    }

    private var controlHeight: CGFloat {
        max(30, self.size.fontSize + (self.size.verticalPadding * 2) + 8)
    }

    private var dropdownPanel: some View {
        ScrollView {
            LazyVStack(spacing: 2) {
                ForEach(self.options, id: \.self) { option in
                    BobeDropdownOptionRow(
                        text: self.label(option),
                        isSelected: option == self.selection
                    ) {
                        self.selection = option
                    }
                }
            }
            .padding(4)
        }
        .frame(maxHeight: min(CGFloat(self.options.count) * 30 + 8, 220))
        .frame(width: self.width ?? 220, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 10)
                .fill(self.theme.colors.surface)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 10)
                .stroke(self.theme.colors.border, lineWidth: 1)
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
        Button(action: self.action) {
            HStack(spacing: 8) {
                Text(self.text)
                    .font(.system(size: 12, weight: self.isSelected ? .semibold : .regular))
                    .foregroundStyle(self.theme.colors.text)
                    .lineLimit(1)
                Spacer(minLength: 0)
                if self.isSelected {
                    Image(systemName: "checkmark")
                        .font(.system(size: 10, weight: .bold))
                        .foregroundStyle(self.theme.colors.primary)
                }
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 6)
            .background(
                RoundedRectangle(cornerRadius: 7)
                    .fill(
                        self.isSelected
                            ? self.theme.colors.primary.opacity(self.theme.isDark ? 0.26 : 0.16)
                            : self.isHovered ? self.theme.colors.background : .clear
                    )
            )
        }
        .buttonStyle(.plain)
        .onHover { self.isHovered = $0 }
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
                .stroke(self.theme.colors.border.opacity(0.6), lineWidth: self.lineWidth)
            Circle()
                .trim(from: 0.12, to: 0.82)
                .stroke(
                    self.color ?? self.theme.colors.primary,
                    style: StrokeStyle(lineWidth: self.lineWidth, lineCap: .round)
                )
                .rotationEffect(.degrees(self.spinning ? 360 : 0))
                .animation(.linear(duration: 0.85).repeatForever(autoreverses: false), value: self.spinning)
        }
        .frame(width: self.size, height: self.size)
        .onAppear { self.spinning = true }
        .onDisappear { self.spinning = false }
    }
}

struct BobeLinearProgressBar: View {
    let progress: Double
    var height: CGFloat = 7

    @Environment(\.theme) private var theme

    private var clampedProgress: Double {
        min(max(self.progress, 0), 1)
    }

    var body: some View {
        GeometryReader { geo in
            ZStack(alignment: .leading) {
                Capsule()
                    .fill(self.theme.colors.border.opacity(0.55))
                Capsule()
                    .fill(
                        LinearGradient(
                            colors: [self.theme.colors.primary, self.theme.colors.secondary],
                            startPoint: .leading,
                            endPoint: .trailing
                        )
                    )
                    .frame(width: max(4, geo.size.width * self.clampedProgress))
            }
        }
        .frame(height: self.height)
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
        Button(action: self.action) {
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
        }
        .buttonStyle(BobePressFeedbackStyle())
        .contentShape(Rectangle())
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

// MARK: - Shared interactive rows

struct BobePressFeedbackStyle: ButtonStyle {
    var pressedOpacity = 0.92
    var pressedScale: CGFloat = 0.995

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .opacity(configuration.isPressed ? self.pressedOpacity : 1)
            .scaleEffect(configuration.isPressed ? self.pressedScale : 1)
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
        Button(action: self.action) {
            HStack(spacing: 10) {
                Image(systemName: self.icon)
                    .font(.system(size: 14))
                    .foregroundStyle(self.isSelected ? self.theme.colors.primary : self.theme.colors.textMuted)
                    .frame(width: 18)

                Text(self.title)
                    .bobeTextStyle(.rowTitle)
                    .fontWeight(self.isSelected ? .semibold : .regular)
                    .foregroundStyle(self.isSelected ? self.theme.colors.primary : self.theme.colors.text)

                Spacer()
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 12)
            .padding(.vertical, 10)
            .background(
                RoundedRectangle(cornerRadius: 8)
                    .fill(self.backgroundColor)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(self.borderColor, lineWidth: self.borderLineWidth)
            )
            .overlay(alignment: .leading) {
                if self.isSelected {
                    RoundedRectangle(cornerRadius: 2)
                        .fill(self.theme.colors.primary)
                        .frame(width: 3)
                        .padding(.vertical, 7)
                }
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(BobePressFeedbackStyle())
        .onHover { self.isHovered = $0 }
    }

    private var backgroundColor: Color {
        if self.isSelected {
            return self.theme.colors.primary.opacity(self.theme.isDark ? 0.24 : 0.14)
        }
        if self.isHovered {
            return self.theme.colors.surface
        }
        return .clear
    }

    private var borderColor: Color {
        if self.isSelected {
            return self.theme.colors.primary.opacity(0.45)
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
