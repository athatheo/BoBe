import SwiftUI

// MARK: - Custom Toggle Switch (36×20px with animated thumb, matching original shared-controls)

struct BobeToggle: View {
    @Binding var isOn: Bool
    @Environment(\.theme) private var theme

    var body: some View {
        Button {
            withAnimation(.spring(duration: 0.2, bounce: 0.1)) {
                isOn.toggle()
            }
        } label: {
            ZStack(alignment: isOn ? .trailing : .leading) {
                Capsule()
                    .fill(isOn ? theme.colors.secondary : theme.colors.border)
                    .frame(width: 36, height: 20)

                Circle()
                    .fill(.white)
                    .frame(width: 16, height: 16)
                    .shadow(color: .black.opacity(0.15), radius: 1, y: 1)
                    .padding(.horizontal, 2)
            }
        }
        .buttonStyle(.plain)
    }
}

// MARK: - Settings Row (label + control)

struct SettingsRow<Content: View>: View {
    let label: String
    var description: String? = nil
    var suffix: String? = nil
    @ViewBuilder let content: Content
    @Environment(\.theme) private var theme

    var body: some View {
        HStack(alignment: .center, spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                Text(label)
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(theme.colors.text)
                if let description {
                    Text(description)
                        .font(.system(size: 11))
                        .foregroundStyle(theme.colors.textMuted)
                }
            }
            Spacer()
            content
            if let suffix {
                Text(suffix)
                    .font(.system(size: 11))
                    .foregroundStyle(theme.colors.textMuted)
            }
        }
    }
}

// MARK: - Debounced Number Input (600ms debounce like original NumInput)

struct DebouncedNumberInput: View {
    @Binding var value: Int
    var range: ClosedRange<Int> = 0...9999
    var width: CGFloat = 80

    @State private var text: String = ""
    @State private var debounceTask: Task<Void, Never>?
    @Environment(\.theme) private var theme

    var body: some View {
        TextField("", text: $text)
            .textFieldStyle(.roundedBorder)
            .frame(width: width)
            .multilineTextAlignment(.trailing)
            .onChange(of: text) { _, newText in
                debounceTask?.cancel()
                debounceTask = Task { @MainActor in
                    try? await Task.sleep(for: .seconds(0.6))
                    if let parsed = Int(newText), range.contains(parsed) {
                        value = parsed
                    }
                }
            }
            .onSubmit {
                debounceTask?.cancel()
                if let parsed = Int(text), range.contains(parsed) {
                    value = parsed
                }
            }
            .onAppear { text = String(value) }
            .onChange(of: value) { _, newVal in
                let str = String(newVal)
                if text != str { text = str }
            }
    }
}

// MARK: - Debounced Decimal Input (for similarity thresholds)

struct DebouncedDecimalInput: View {
    @Binding var value: Double
    var range: ClosedRange<Double> = 0...1
    var step: Double = 0.05
    var width: CGFloat = 80

    @State private var text: String = ""
    @State private var debounceTask: Task<Void, Never>?

    var body: some View {
        TextField("", text: $text)
            .textFieldStyle(.roundedBorder)
            .frame(width: width)
            .multilineTextAlignment(.trailing)
            .onChange(of: text) { _, newText in
                debounceTask?.cancel()
                debounceTask = Task { @MainActor in
                    try? await Task.sleep(for: .seconds(0.6))
                    if let parsed = Double(newText), range.contains(parsed) {
                        value = parsed
                    }
                }
            }
            .onAppear { text = String(format: "%.2f", value) }
            .onChange(of: value) { _, newVal in
                let str = String(format: "%.2f", newVal)
                if text != str { text = str }
            }
    }
}

// MARK: - Collapsible Settings Section (matching original Section with toggle)

struct CollapsibleSection<Content: View>: View {
    let title: String
    let icon: String
    var description: String? = nil
    var toggleBinding: Binding<Bool>? = nil
    @ViewBuilder let content: Content

    @State private var isExpanded = true
    @Environment(\.theme) private var theme

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Header
            Button {
                withAnimation(.easeInOut(duration: 0.2)) {
                    isExpanded.toggle()
                }
            } label: {
                HStack(spacing: 10) {
                    Image(systemName: icon)
                        .font(.system(size: 14))
                        .foregroundStyle(theme.colors.primary)
                        .frame(width: 20)

                    VStack(alignment: .leading, spacing: 2) {
                        Text(title)
                            .font(.system(size: 14, weight: .semibold))
                            .foregroundStyle(theme.colors.text)
                        if let description {
                            Text(description)
                                .font(.system(size: 11))
                                .foregroundStyle(theme.colors.textMuted)
                        }
                    }

                    Spacer()

                    if let binding = toggleBinding {
                        BobeToggle(isOn: binding)
                    }

                    Image(systemName: isExpanded ? "chevron.up" : "chevron.down")
                        .font(.system(size: 10))
                        .foregroundStyle(theme.colors.textMuted)
                }
                .padding(.vertical, 8)
            }
            .buttonStyle(.plain)

            // Content
            if isExpanded {
                VStack(alignment: .leading, spacing: 12) {
                    content
                }
                .padding(.leading, 30)
                .padding(.top, 4)
                .padding(.bottom, 12)
                .transition(.opacity.combined(with: .move(edge: .top)))
            }
        }
    }
}

// MARK: - Format Helpers

func formatBytes(_ bytes: Int) -> String {
    let gb = Double(bytes) / 1_073_741_824
    if gb >= 1 { return String(format: "%.1f GB", gb) }
    let mb = Double(bytes) / 1_048_576
    if mb >= 1 { return String(format: "%.0f MB", mb) }
    let kb = Double(bytes) / 1024
    return String(format: "%.0f KB", kb)
}
