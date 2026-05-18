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

/// Top-aligned icon + title + description + indented body. Pattern used by
/// every "flat" (non-collapsible) settings section so the engine flow
/// doesn't look visually scrambled between Disclosure-style siblings.
struct FlatSettingsSection<Content: View>: View {
    let icon: String
    let title: String
    let description: String?
    @ViewBuilder let content: Content
    @Environment(\.theme) private var theme

    init(
        icon: String,
        title: String,
        description: String? = nil,
        @ViewBuilder content: () -> Content
    ) {
        self.icon = icon
        self.title = title
        self.description = description
        self.content = content()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 10) {
                Image(systemName: self.icon)
                    .font(.system(size: 14))
                    .foregroundStyle(self.theme.colors.primary)
                    .frame(width: 20)
                VStack(alignment: .leading, spacing: 2) {
                    Text(self.title)
                        .bobeTextStyle(.heading)
                        .foregroundStyle(self.theme.colors.text)
                    if let description = self.description {
                        Text(description)
                            .bobeTextStyle(.helper)
                            .foregroundStyle(self.theme.colors.textMuted)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
                Spacer()
            }
            VStack(alignment: .leading, spacing: 10) {
                self.content
            }
            .padding(.leading, 30)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// Red exclamation banner used at the top of every settings panel to
/// surface a persist/load failure. Extracted from EnginePanel / VoicePanel /
/// BehaviorPanel which had byte-identical inline copies.
struct SettingsErrorBanner: View {
    let message: String
    @Environment(\.theme) private var theme

    var body: some View {
        HStack(spacing: 6) {
            Image(systemName: "exclamationmark.triangle.fill")
                .foregroundStyle(self.theme.colors.primary)
            Text(self.message)
                .bobeTextStyle(.body)
                .foregroundStyle(self.theme.colors.primary)
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 8).fill(self.theme.colors.primary.opacity(0.08))
        )
    }
}

/// "Saved ✓" toast variant used by the same panels. Auto-dismiss timing
/// lives on `SettingsDebouncer.scheduleToastClear` — this view is presentation only.
struct SettingsSavedToast: View {
    let message: String
    @Environment(\.theme) private var theme

    var body: some View {
        HStack(spacing: 6) {
            Image(systemName: "checkmark.circle.fill")
                .foregroundStyle(self.theme.colors.secondary)
            Text(self.message)
                .bobeTextStyle(.helper)
                .foregroundStyle(self.theme.colors.secondary)
            Spacer()
        }
        .transition(.opacity)
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
        // HStack restructure rationale:
        //   Previously this row used a Grid where BOTH columns asked for
        //   `.frame(maxWidth: .infinity)` — distribution was unpredictable
        //   and greedy controls (BobeTextField with `width: nil`) could
        //   squeeze the label to nothing, or long descriptions could push
        //   the control out of view.
        //
        //   Today: label-column has `.frame(maxWidth: .infinity)` so it
        //   absorbs remaining width; control-column has no `.infinity`
        //   so it sits at its intrinsic size. The control wins width
        //   negotiation by virtue of being non-flexible (intrinsic
        //   always beats flexible at SwiftUI's layout pass when both are
        //   in a non-flexible parent).
        //
        //   `.fixedSize(horizontal: false, vertical: true)` on the text
        //   lets descriptions wrap freely within the label column's
        //   allotted width. `alignment: .top` so a wrapped description
        //   doesn't push the control row down vertically.
        //
        //   Callers that put a greedy control (TextField with no width)
        //   into the content slot will still distort the row — they need
        //   to pass an explicit width to BobeTextField. Most usages
        //   already do; the few that didn't were fixed alongside this
        //   restructure (e.g. EnginePanel.localProviderSection).
        HStack(alignment: .top, spacing: 16) {
            VStack(alignment: .leading, spacing: 2) {
                Text(self.label)
                    .bobeTextStyle(.body)
                    .fontWeight(.medium)
                    .foregroundStyle(self.theme.colors.text)
                    .fixedSize(horizontal: false, vertical: true)
                if let description {
                    Text(description)
                        .bobeTextStyle(.helper)
                        .foregroundStyle(self.theme.colors.textMuted)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)

            HStack(alignment: .center, spacing: 8) {
                self.content
                if let suffix {
                    Text(suffix)
                        .bobeTextStyle(.helper)
                        .foregroundStyle(self.theme.colors.textMuted)
                        .fixedSize(horizontal: true, vertical: false)
                }
            }
            .fixedSize(horizontal: true, vertical: false)
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

struct CollapsibleSection<Content: View>: View {
    let title: String
    let icon: String
    var description: String?
    var toggleBinding: Binding<Bool>?
    var aiCuratedBadge = false
    var initiallyExpanded = true
    @ViewBuilder let content: Content

    @State private var isExpanded: Bool
    @Environment(\.theme) private var theme

    init(
        title: String,
        icon: String,
        description: String? = nil,
        toggleBinding: Binding<Bool>? = nil,
        aiCuratedBadge: Bool = false,
        initiallyExpanded: Bool = true,
        @ViewBuilder content: () -> Content
    ) {
        self.title = title
        self.icon = icon
        self.description = description
        self.toggleBinding = toggleBinding
        self.aiCuratedBadge = aiCuratedBadge
        self.initiallyExpanded = initiallyExpanded
        self.content = content()
        self._isExpanded = State(initialValue: initiallyExpanded)
    }

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
                    HStack(spacing: 6) {
                        Text(self.title)
                            .bobeTextStyle(.heading)
                            .foregroundStyle(self.theme.colors.text)
                        if self.aiCuratedBadge {
                            Text(L10n.tr("settings.goals.section.ai_curated_badge"))
                                .bobeTextStyle(.badge)
                                .foregroundStyle(self.theme.colors.tertiary)
                                .padding(.horizontal, 5)
                                .padding(.vertical, 1)
                                .background(
                                    Capsule().fill(self.theme.colors.tertiary.opacity(0.15))
                                )
                                .overlay(
                                    Capsule().stroke(self.theme.colors.tertiary.opacity(0.4), lineWidth: 0.5)
                                )
                        }
                    }
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
        .animation(OverlayMotionRuntime.reduceMotion ? nil : .easeInOut(duration: 0.2), value: self.isExpanded)
    }
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
