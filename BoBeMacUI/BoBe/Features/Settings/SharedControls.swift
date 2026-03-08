import SwiftUI

struct BobeToggle: View {
    @Binding var isOn: Bool
    var accessibilityLabel: String?
    @Environment(\.theme) private var theme

    init(isOn: Binding<Bool>, accessibilityLabel: String? = nil) {
        _isOn = isOn
        self.accessibilityLabel = accessibilityLabel
    }

    var body: some View {
        Toggle("", isOn: self.$isOn)
            .labelsHidden()
            .toggleStyle(.switch)
            .tint(self.theme.colors.secondary)
            .controlSize(.small)
            .accessibilityLabel(Text(self.accessibilityLabel ?? L10n.tr("settings.shared.toggle.enabled")))
    }
}

struct AccentAddButton: View {
    var title = L10n.tr("settings.shared.action.add_new")
    let action: () -> Void

    var body: some View {
        Button(self.title, action: self.action)
            .bobeButton(.primary, size: .small)
    }
}

struct SettingsPaneHeader: View {
    let title: String
    var actionTitle = L10n.tr("settings.shared.action.add_new")
    let action: () -> Void
    @Environment(\.theme) private var theme

    var body: some View {
        HStack {
            Text(self.title)
                .font(.headline)
                .foregroundStyle(self.theme.colors.text)
            Spacer()
            AccentAddButton(title: self.actionTitle, action: self.action)
        }
    }
}

struct SettingsRow<Content: View>: View {
    let label: String
    var description: String?
    var suffix: String?
    @ViewBuilder let content: Content
    @Environment(\.theme) private var theme

    var body: some View {
        HStack(alignment: .center, spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                Text(self.label)
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(self.theme.colors.text)
                if let description {
                    Text(description)
                        .font(.system(size: 11))
                        .foregroundStyle(self.theme.colors.textMuted)
                }
            }
            Spacer()
            self.content
            if let suffix {
                Text(suffix)
                    .font(.system(size: 11))
                    .foregroundStyle(self.theme.colors.textMuted)
            }
        }
    }
}

struct ThemedSplitPane<Left: View, Right: View>: View {
    let leftWidth: CGFloat
    @ViewBuilder let left: Left
    @ViewBuilder let right: Right
    @Environment(\.theme) private var theme

    var body: some View {
        HStack(spacing: 0) {
            self.left
                .frame(width: self.leftWidth)
            Rectangle()
                .fill(self.theme.colors.border)
                .frame(width: 1)
            self.right
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }
}

struct DebouncedNumberInput: View {
    @Binding var value: Int
    var range: ClosedRange<Int> = 0 ... 9999
    var width: CGFloat = 80

    @State private var text = ""
    @State private var debounceTask: Task<Void, Never>?
    @FocusState private var isFocused: Bool
    @State private var isHovered = false
    @Environment(\.theme) private var theme

    var body: some View {
        TextField("", text: self.$text)
            .textFieldStyle(.plain)
            .font(.system(size: 13, weight: .medium, design: .monospaced))
            .multilineTextAlignment(.trailing)
            .foregroundStyle(self.theme.colors.text)
            .tint(self.theme.colors.primary)
            .focused(self.$isFocused)
            .bobeInputChrome(focused: self.isFocused, hovered: self.isHovered)
            .onHover { self.isHovered = $0 }
            .frame(width: self.width)
            .onChange(of: self.text) { _, newText in
                guard self.isFocused else { return }
                self.debounceTask?.cancel()
                self.debounceTask = Task { @MainActor in
                    try? await Task.sleep(for: .seconds(0.6))
                    if let parsed = Int(newText), range.contains(parsed) {
                        self.value = parsed
                    }
                }
            }
            .onSubmit {
                self.debounceTask?.cancel()
                if let parsed = Int(text), range.contains(parsed) {
                    self.value = parsed
                }
            }
            .onAppear { self.text = String(self.value) }
            .onChange(of: self.value) { _, newVal in
                guard !self.isFocused else { return }
                let str = String(newVal)
                if self.text != str { self.text = str }
            }
    }
}

struct DebouncedDecimalInput: View {
    @Binding var value: Double
    var range: ClosedRange<Double> = 0 ... 1
    var step = 0.05
    var width: CGFloat = 80

    @State private var text = ""
    @State private var debounceTask: Task<Void, Never>?
    @FocusState private var isFocused: Bool
    @State private var isHovered = false
    @Environment(\.theme) private var theme

    var body: some View {
        TextField("", text: self.$text)
            .textFieldStyle(.plain)
            .font(.system(size: 13, weight: .medium, design: .monospaced))
            .multilineTextAlignment(.trailing)
            .foregroundStyle(self.theme.colors.text)
            .tint(self.theme.colors.primary)
            .focused(self.$isFocused)
            .bobeInputChrome(focused: self.isFocused, hovered: self.isHovered)
            .onHover { self.isHovered = $0 }
            .frame(width: self.width)
            .onChange(of: self.text) { _, newText in
                guard self.isFocused else { return }
                self.debounceTask?.cancel()
                self.debounceTask = Task { @MainActor in
                    try? await Task.sleep(for: .seconds(0.6))
                    if let parsed = Double(newText), range.contains(parsed) {
                        self.value = parsed
                    }
                }
            }
            .onAppear { self.text = String(format: "%.2f", self.value) }
            .onChange(of: self.value) { _, newVal in
                guard !self.isFocused else { return }
                let str = String(format: "%.2f", newVal)
                if self.text != str { self.text = str }
            }
    }
}

struct CollapsibleSection<Content: View>: View {
    let title: String
    let icon: String
    var description: String?
    var toggleBinding: Binding<Bool>?
    @ViewBuilder let content: Content

    @State private var isExpanded = true
    @Environment(\.theme) private var theme

    var body: some View {
        DisclosureGroup(isExpanded: self.$isExpanded) {
            VStack(alignment: .leading, spacing: 0) {
                let isDisabled = self.toggleBinding.map { !$0.wrappedValue } ?? false
                VStack(alignment: .leading, spacing: 12) {
                    self.content
                }
                .padding(.leading, 30)
                .padding(.top, 4)
                .padding(.bottom, 12)
                .disabled(isDisabled)
                .opacity(isDisabled ? 0.5 : 1)
                .transition(.opacity.combined(with: .move(edge: .top)))
            }
        } label: {
            HStack(spacing: 10) {
                Image(systemName: self.icon)
                    .font(.system(size: 14))
                    .foregroundStyle(self.theme.colors.primary)
                    .frame(width: 20)

                VStack(alignment: .leading, spacing: 2) {
                    Text(self.title)
                        .font(.system(size: 14, weight: .semibold))
                        .foregroundStyle(self.theme.colors.text)
                    if let description {
                        Text(description)
                            .font(.system(size: 11))
                            .foregroundStyle(self.theme.colors.textMuted)
                    }
                }

                Spacer()

                if let toggleBinding {
                    BobeToggle(
                        isOn: toggleBinding,
                        accessibilityLabel: L10n.tr("settings.shared.section.enabled_format", self.title)
                    )
                }
            }
            .padding(.vertical, 8)
            .padding(.horizontal, 8)
            .background(
                RoundedRectangle(cornerRadius: 8)
                    .fill(self.isExpanded ? self.theme.colors.surface : .clear)
            )
        }
        .tint(self.theme.colors.primary)
        .animation(.easeInOut(duration: 0.2), value: self.isExpanded)
    }
}

func formatBytes(_ bytes: Int) -> String {
    let gb = Double(bytes) / 1_073_741_824
    if gb >= 1 { return L10n.tr("settings.shared.bytes.gb_format", gb) }
    let mb = Double(bytes) / 1_048_576
    if mb >= 1 { return L10n.tr("settings.shared.bytes.mb_format", mb) }
    let kb = Double(bytes) / 1024
    return L10n.tr("settings.shared.bytes.kb_format", kb)
}

#if !SPM_BUILD
#Preview("BobeToggle") {
    @Previewable @State var isOn = true
    HStack(spacing: 20) {
        BobeToggle(isOn: $isOn)
        Text(isOn ? "On" : "Off")
    }
    .environment(\.theme, allThemes[0])
    .padding()
}

#Preview("SettingsRow") {
    @Previewable @State var toggle = true
    VStack(spacing: 16) {
        SettingsRow(label: "Enable Feature", description: "A helpful description") {
            BobeToggle(isOn: $toggle)
        }
        SettingsRow(label: "Token Limit", suffix: "tokens") {
            Text("4096")
                .font(.system(size: 13, design: .monospaced))
        }
    }
    .environment(\.theme, allThemes[0])
    .padding()
    .frame(width: 400)
}

#Preview("CollapsibleSection") {
    @Previewable @State var toggle = true
    CollapsibleSection(title: "Screen Capture", icon: "camera.fill", description: "Periodic screenshots", toggleBinding: $toggle) {
        Text("Section content goes here")
    }
    .environment(\.theme, allThemes[0])
    .padding()
    .frame(width: 400)
}

#Preview("DebouncedNumberInput") {
    @Previewable @State var value = 4096
    SettingsRow(label: "Max Tokens") {
        DebouncedNumberInput(value: $value, range: 1 ... 8192)
    }
    .environment(\.theme, allThemes[0])
    .padding()
    .frame(width: 400)
}
#endif
