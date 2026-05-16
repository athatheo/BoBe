import SwiftUI

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
