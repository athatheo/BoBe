import CoreGraphics
import SwiftUI

/// Behavior settings panel — capture, check-ins, memory, conversation, tools.
struct BehaviorPanel: View {
    @State private var settings: DaemonSettings?
    @State private var isLoading = false
    @State private var isSaving = false
    @State private var error: String?
    @State private var newCheckinTime = ""
    @State private var saveTask: Task<Void, Never>?
    @Environment(\.theme) private var theme

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                if let error {
                    HStack(spacing: 6) {
                        Image(systemName: "exclamationmark.triangle.fill")
                            .foregroundStyle(self.theme.colors.primary)
                        Text(error)
                            .font(.system(size: 12))
                            .foregroundStyle(self.theme.colors.primary)
                    }
                    .padding(10)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(RoundedRectangle(cornerRadius: 8).fill(self.theme.colors.primary.opacity(0.08)))
                }

                if self.settings != nil {
                    self.captureSection
                    self.checkinSection
                    self.memorySection
                    self.conversationSection
                    self.toolsSection
                } else if self.isLoading {
                    HStack(spacing: 8) {
                        BobeSpinner(size: 14)
                        Text("Loading behavior settings...")
                            .font(.system(size: 13))
                            .foregroundStyle(self.theme.colors.textMuted)
                    }
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.top, 40)
                }
            }
            .padding(24)
        }
        .task { await self.loadSettings() }
    }

    private var captureSection: some View {
        CollapsibleSection(
            title: "Screen Capture",
            icon: "camera.fill",
            description: "BoBe periodically captures your screen for context",
            toggleBinding: self.binding(\.captureEnabled, fallback: false)
        ) {
            SettingsRow(label: "Capture interval", suffix: "seconds") {
                DebouncedNumberInput(value: self.binding(\.captureIntervalSeconds, fallback: 60), range: 1 ... 600)
            }

            if !CGPreflightScreenCaptureAccess() {
                HStack(spacing: 6) {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .foregroundStyle(self.theme.colors.tertiary)
                    Text("Screen Recording permission not granted.")
                        .font(.system(size: 12))
                        .foregroundStyle(self.theme.colors.tertiary)
                    Button("Open Settings") {
                        if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture") {
                            NSWorkspace.shared.open(url)
                        }
                    }
                    .font(.system(size: 11))
                    .bobeButton(.secondary, size: .mini)
                }
            }
        }
    }

    private var checkinSection: some View {
        CollapsibleSection(
            title: "Check-ins",
            icon: "clock.fill",
            description: "Scheduled proactive check-ins throughout the day",
            toggleBinding: self.binding(\.checkinEnabled, fallback: false)
        ) {
            SettingsRow(label: "Schedule") {
                EmptyView()
            }
            FlowLayout(spacing: 6) {
                ForEach(self.settings?.checkinTimes ?? [], id: \.self) { time in
                    HStack(spacing: 4) {
                        Text(time)
                            .font(.system(size: 11, design: .monospaced))
                        Button {
                            self.removeCheckinTime(time)
                        } label: {
                            Image(systemName: "xmark.circle.fill")
                                .font(.system(size: 10))
                                .foregroundStyle(self.theme.colors.textMuted)
                        }
                        .bobeButton(.ghost, size: .mini)
                    }
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(Capsule().fill(self.theme.colors.border.opacity(0.5)))
                }
            }

            HStack(spacing: 6) {
                BobeTextField(placeholder: "HH:MM", text: self.$newCheckinTime, width: 80) {
                    self.addCheckinTime()
                }
                Button("Add") { self.addCheckinTime() }
                    .bobeButton(.secondary, size: .small)
                    .disabled(self.newCheckinTime.isEmpty)
            }

            SettingsRow(label: "Jitter", suffix: "minutes") {
                DebouncedNumberInput(value: self.binding(\.checkinJitterMinutes, fallback: 0), range: 0 ... 30)
            }
        }
    }

    private var memorySection: some View {
        CollapsibleSection(
            title: "Memory",
            icon: "brain.head.profile",
            description: "How long BoBe retains memories",
            toggleBinding: self.binding(\.learningEnabled, fallback: true)
        ) {
            SettingsRow(label: "Short-term retention", suffix: "days") {
                DebouncedNumberInput(value: self.binding(\.memoryShortTermRetentionDays, fallback: 7), range: 1 ... 365)
            }
            SettingsRow(label: "Long-term retention", suffix: "days") {
                DebouncedNumberInput(value: self.binding(\.memoryLongTermRetentionDays, fallback: 365), range: 1 ... 3650)
            }
        }
    }

    private var conversationSection: some View {
        CollapsibleSection(
            title: "Conversation",
            icon: "message.fill",
            description: "How conversations are managed"
        ) {
            SettingsRow(label: "Auto-close after", suffix: "minutes") {
                DebouncedNumberInput(value: self.binding(\.conversationAutoCloseMinutes, fallback: 10), range: 1 ... 60)
            }
            SettingsRow(label: "Generate summaries") {
                BobeToggle(isOn: self.binding(\.conversationSummaryEnabled, fallback: true))
            }
        }
    }

    private var toolsSection: some View {
        CollapsibleSection(
            title: "Tools",
            icon: "wrench.fill",
            description: "Allow BoBe to execute actions on your behalf",
            toggleBinding: self.binding(\.toolsEnabled, fallback: true)
        ) {
            SettingsRow(label: "Max iterations", suffix: "rounds") {
                DebouncedNumberInput(value: self.binding(\.toolsMaxIterations, fallback: 8), range: 1 ... 20)
            }
        }
    }

    // MARK: - Helpers

    private func binding<V>(
        _ keyPath: WritableKeyPath<DaemonSettings, V>,
        fallback: @autoclosure @escaping () -> V
    ) -> Binding<V> {
        Binding(
            get: {
                settings?[keyPath: keyPath] ?? fallback()
            },
            set: { newValue in
                guard var current = settings else { return }
                current[keyPath: keyPath] = newValue
                self.settings = current
                self.debounceSave()
            }
        )
    }

    private func debounceSave() {
        self.saveTask?.cancel()
        self.isSaving = true
        let currentSettings = self.settings
        self.saveTask = Task {
            try? await Task.sleep(for: .seconds(0.6))
            guard !Task.isCancelled, let currentSettings else {
                self.isSaving = false
                return
            }
            do {
                var req = SettingsUpdateRequest()
                req.captureEnabled = currentSettings.captureEnabled
                req.captureIntervalSeconds = currentSettings.captureIntervalSeconds
                req.checkinEnabled = currentSettings.checkinEnabled
                req.checkinTimes = currentSettings.checkinTimes
                req.checkinJitterMinutes = currentSettings.checkinJitterMinutes
                req.learningEnabled = currentSettings.learningEnabled
                req.memoryShortTermRetentionDays = currentSettings.memoryShortTermRetentionDays
                req.memoryLongTermRetentionDays = currentSettings.memoryLongTermRetentionDays
                req.conversationAutoCloseMinutes = currentSettings.conversationAutoCloseMinutes
                req.conversationSummaryEnabled = currentSettings.conversationSummaryEnabled
                req.toolsEnabled = currentSettings.toolsEnabled
                req.toolsMaxIterations = currentSettings.toolsMaxIterations
                _ = try await DaemonClient.shared.updateSettings(req)
                self.error = nil
            } catch {
                self.error = error.localizedDescription
            }
            self.isSaving = false
        }
    }

    private func addCheckinTime() {
        guard !self.newCheckinTime.isEmpty else { return }
        let trimmed = self.newCheckinTime.trimmingCharacters(in: .whitespaces)
        guard !(self.settings?.checkinTimes.contains(trimmed) ?? false) else { return }
        self.settings?.checkinTimes.append(trimmed)
        self.newCheckinTime = ""
        self.debounceSave()
    }

    private func removeCheckinTime(_ time: String) {
        self.settings?.checkinTimes.removeAll { $0 == time }
        self.debounceSave()
    }

    private func loadSettings() async {
        self.isLoading = true
        defer { isLoading = false }
        do {
            self.settings = try await DaemonClient.shared.getSettings()
        } catch {
            self.error = error.localizedDescription
        }
    }
}

// MARK: - Shared Components

/// Simple flow layout for pills/tags
struct FlowLayout: Layout {
    let spacing: CGFloat

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let result = self.arrange(proposal: proposal, subviews: subviews)
        return result.size
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let result = self.arrange(proposal: proposal, subviews: subviews)
        for (index, subview) in subviews.enumerated() {
            subview.place(at: CGPoint(x: bounds.minX + result.positions[index].x, y: bounds.minY + result.positions[index].y), proposal: .unspecified)
        }
    }

    private func arrange(proposal: ProposedViewSize, subviews: Subviews) -> (size: CGSize, positions: [CGPoint]) {
        let maxWidth = proposal.width ?? .infinity
        var positions: [CGPoint] = []
        var x: CGFloat = 0
        var y: CGFloat = 0
        var rowHeight: CGFloat = 0
        var totalHeight: CGFloat = 0

        for subview in subviews {
            let size = subview.sizeThatFits(.unspecified)
            if x + size.width > maxWidth, x > 0 {
                x = 0
                y += rowHeight + self.spacing
                rowHeight = 0
            }
            positions.append(CGPoint(x: x, y: y))
            rowHeight = max(rowHeight, size.height)
            x += size.width + self.spacing
            totalHeight = y + rowHeight
        }

        return (CGSize(width: maxWidth, height: totalHeight), positions)
    }
}
