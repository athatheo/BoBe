import CoreGraphics
import SwiftUI

struct BehaviorPanel: View {
    @State private var settings: DaemonSettings?
    @State private var isLoading = false
    @State private var isSaving = false
    @State private var error: String?
    @State private var savedMessage: String?
    @State private var newCheckinTime = ""
    @State private var debouncer = SettingsDebouncer()
    @State private var restartFields: Set<String> = []
    @State private var bannerDismissed = false
    @Environment(\.theme) private var theme

    /// CheckinScheduler captures these at boot and only re-reads on restart.
    private static let deferToRestartFields: Set<String> = [
        "checkin_enabled",
        "checkin_times",
        "checkin_jitter_minutes",
    ]

    private var visibleRestartFields: Set<String> {
        self.bannerDismissed ? [] : self.restartFields
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                if !self.visibleRestartFields.isEmpty {
                    RestartRequiredBanner(
                        fields: self.visibleRestartFields,
                        onDismiss: { self.bannerDismissed = true }
                    )
                }

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

                if let savedMessage {
                    HStack(spacing: 6) {
                        Image(systemName: "checkmark.circle.fill")
                            .foregroundStyle(self.theme.colors.secondary)
                        Text(savedMessage)
                            .font(.system(size: 11))
                            .foregroundStyle(self.theme.colors.secondary)
                        Spacer()
                    }
                    .transition(.opacity)
                }

                if self.settings != nil {
                    self.captureSection
                    self.checkinSection
                    self.conversationSection
                } else if self.isLoading {
                    HStack(spacing: 8) {
                        BobeSpinner(size: 14)
                        Text(L10n.tr("settings.behavior.loading"))
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
            title: L10n.tr("settings.behavior.capture.title"),
            icon: "camera.fill",
            description: L10n.tr("settings.behavior.capture.description"),
            toggleBinding: self.binding(\.captureEnabled, fallback: false)
        ) {
            SettingsRow(label: L10n.tr("settings.behavior.capture.interval"), suffix: L10n.tr("settings.units.seconds")) {
                DebouncedNumberInput(value: self.binding(\.captureIntervalSeconds, fallback: 60), range: 1 ... 600)
            }

            if !CGPreflightScreenCaptureAccess() {
                HStack(spacing: 6) {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .foregroundStyle(self.theme.colors.tertiary)
                    Text(L10n.tr("settings.behavior.capture.permission_missing"))
                        .font(.system(size: 12))
                        .foregroundStyle(self.theme.colors.tertiary)
                    Button(L10n.tr("setup.permissions.open_settings")) {
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
            title: L10n.tr("settings.behavior.checkins.title"),
            icon: "clock.fill",
            description: L10n.tr("settings.behavior.checkins.description"),
            toggleBinding: self.binding(\.checkinEnabled, fallback: false)
        ) {
            SettingsRow(label: L10n.tr("settings.behavior.checkins.schedule")) {
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
                BobeTextField(placeholder: L10n.tr("settings.behavior.checkins.time_placeholder"), text: self.$newCheckinTime, width: 80) {
                    self.addCheckinTime()
                }
                Button(L10n.tr("settings.behavior.checkins.action.add")) { self.addCheckinTime() }
                    .bobeButton(.secondary, size: .small)
                    .disabled(self.newCheckinTime.isEmpty)
            }

            SettingsRow(label: L10n.tr("settings.behavior.checkins.jitter"), suffix: L10n.tr("settings.units.minutes")) {
                DebouncedNumberInput(value: self.binding(\.checkinJitterMinutes, fallback: 0), range: 0 ... 30)
            }
        }
    }

    private var conversationSection: some View {
        CollapsibleSection(
            title: L10n.tr("settings.behavior.conversation.title"),
            icon: "message.fill",
            description: L10n.tr("settings.behavior.conversation.description")
        ) {
            SettingsRow(
                label: L10n.tr("settings.behavior.conversation.auto_close_after"),
                description: L10n.tr("settings.behavior.conversation.auto_close.description"),
                suffix: L10n.tr("settings.units.minutes")
            ) {
                DebouncedNumberInput(value: self.binding(\.conversationAutoCloseMinutes, fallback: 10), range: 1 ... 60)
            }
        }
    }

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
                self.debounceSave(touched: Self.fieldKey(for: keyPath))
            }
        )
    }

    private static func fieldKey<V>(for keyPath: WritableKeyPath<DaemonSettings, V>) -> String? {
        switch keyPath {
        case \DaemonSettings.captureEnabled: "capture_enabled"
        case \DaemonSettings.captureIntervalSeconds: "capture_interval_seconds"
        case \DaemonSettings.checkinEnabled: "checkin_enabled"
        case \DaemonSettings.checkinTimes: "checkin_times"
        case \DaemonSettings.checkinJitterMinutes: "checkin_jitter_minutes"
        case \DaemonSettings.conversationAutoCloseMinutes: "conversation_auto_close_minutes"
        default: nil
        }
    }

    private func debounceSave(touched: String? = nil) {
        self.isSaving = true
        let currentSettings = self.settings
        let touchedFields = self.collectTouchedFields(initial: touched)
        self.debouncer.debounce {
            guard let currentSettings else {
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
                req.conversationAutoCloseMinutes = currentSettings.conversationAutoCloseMinutes

                let resp = try await DaemonClient.shared.updateSettings(req)
                self.applySaveResponse(touched: touchedFields, response: resp)
            } catch {
                self.error = error.localizedDescription
                self.savedMessage = nil
            }
            self.isSaving = false
        }
    }

    private func applySaveResponse(touched: Set<String>, response: SettingsUpdateResponse) {
        if response.persistFailed == true {
            self.error = L10n.tr("settings.shared.action.persist_failed")
            self.savedMessage = nil
        } else {
            self.error = nil
            self.savedMessage = response.message
            self.scheduleSavedToastDismiss()
        }
        self.applyRestartFields(touched: touched, response: response)
    }

    private func scheduleSavedToastDismiss() {
        self.debouncer.scheduleToastClear { self.savedMessage = nil }
    }

    private func collectTouchedFields(initial: String?) -> Set<String> {
        var fields: Set<String> = []
        if let initial { fields.insert(initial) }
        return fields
    }

    private func applyRestartFields(touched: Set<String>, response: SettingsUpdateResponse) {
        let shadowed = touched.intersection(Self.deferToRestartFields)
        let daemonReported = Set(response.restartRequiredFields)
        let combined = shadowed.union(daemonReported)
        if combined.isEmpty { return }
        self.restartFields = self.restartFields.union(combined)
        self.bannerDismissed = false
    }

    private func addCheckinTime() {
        guard !self.newCheckinTime.isEmpty else { return }
        let trimmed = self.newCheckinTime.trimmingCharacters(in: .whitespaces)
        guard !(self.settings?.checkinTimes.contains(trimmed) ?? false) else { return }
        self.settings?.checkinTimes.append(trimmed)
        self.newCheckinTime = ""
        self.debounceSave(touched: "checkin_times")
    }

    private func removeCheckinTime(_ time: String) {
        self.settings?.checkinTimes.removeAll { $0 == time }
        self.debounceSave(touched: "checkin_times")
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
