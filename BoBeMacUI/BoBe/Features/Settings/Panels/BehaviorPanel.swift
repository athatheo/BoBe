import CoreGraphics
import SwiftUI

struct BehaviorPanel: View {
    @State private var store = SettingsStore.shared
    @State private var newCheckinTime: Date = Calendar.current.date(
        bySettingHour: 9, minute: 0, second: 0, of: .now
    ) ?? .now
    @State private var bannerDismissed = false
    @Environment(\.theme) private var theme

    /// Format check-in time as "HH:mm" for the daemon. Pinning to POSIX keeps
    /// the wire format stable across locales (e.g., 24h vs 12h users).
    private static let timeFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateFormat = "HH:mm"
        formatter.locale = Locale(identifier: "en_US_POSIX")
        return formatter
    }()

    private var visibleRestartFields: Set<String> {
        self.bannerDismissed ? [] : self.store.restartFields
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                if !self.visibleRestartFields.isEmpty {
                    RestartRequiredBanner(
                        fields: self.visibleRestartFields,
                        onDismiss: {
                            self.bannerDismissed = true
                            self.store.clearRestartFields()
                        }
                    )
                }

                if let error = self.store.error {
                    SettingsErrorBanner(message: error)
                }

                if let savedMessage = self.store.savedMessage {
                    SettingsSavedToast(message: savedMessage)
                }

                if self.store.settings != nil {
                    self.captureSection
                    self.checkinSection
                    self.conversationSection
                    self.goalsSection
                } else if self.store.isLoading {
                    HStack(spacing: 8) {
                        BobeSpinner(size: 14)
                        Text(L10n.tr("settings.behavior.loading"))
                            .bobeTextStyle(.settingsBody)
                            .foregroundStyle(self.theme.colors.textMuted)
                    }
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.top, 40)
                }
            }
            .padding(24)
        }
        .task { await self.store.loadIfNeeded() }
        .onDisappear { self.store.cancelToast() }
    }

    private var captureSection: some View {
        CollapsibleSection(
            title: L10n.tr("settings.behavior.capture.title"),
            icon: "camera.fill",
            description: L10n.tr("settings.behavior.capture.description"),
            toggleBinding: self.store.binding(\.captureEnabled, fallback: false, touched: "capture_enabled")
        ) {
            SettingsRow(label: L10n.tr("settings.behavior.capture.interval"), suffix: L10n.tr("settings.units.seconds")) {
                DebouncedNumberInput(
                    value: self.store.binding(\.captureIntervalSeconds, fallback: 60, touched: "capture_interval_seconds"),
                    range: 1 ... 600
                )
            }

            if !CGPreflightScreenCaptureAccess() {
                HStack(spacing: 6) {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .foregroundStyle(self.theme.colors.tertiary)
                    Text(L10n.tr("settings.behavior.capture.permission_missing"))
                        .bobeTextStyle(.helper)
                        .foregroundStyle(self.theme.colors.tertiary)
                        .fixedSize(horizontal: false, vertical: true)
                    Spacer(minLength: 8)
                    Button(L10n.tr("setup.permissions.open_settings")) {
                        if let url = URL(string: "x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?Privacy_ScreenCapture") {
                            NSWorkspace.shared.open(url)
                        }
                    }
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
            toggleBinding: self.store.binding(\.checkinEnabled, fallback: false, touched: "checkin_enabled")
        ) {
            SettingsRow(label: L10n.tr("settings.behavior.checkins.schedule")) {
                EmptyView()
            }
            FlowLayout(spacing: 6) {
                ForEach(self.store.settings?.checkinTimes ?? [], id: \.self) { time in
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
                DatePicker(
                    "",
                    selection: self.$newCheckinTime,
                    displayedComponents: .hourAndMinute
                )
                .labelsHidden()
                .datePickerStyle(.compact)
                .accessibilityLabel(L10n.tr("settings.behavior.checkins.time_placeholder"))
                Button(L10n.tr("settings.behavior.checkins.action.add")) { self.addCheckinTime() }
                    .bobeButton(.secondary, size: .small)
            }

            SettingsRow(label: L10n.tr("settings.behavior.checkins.jitter"), suffix: L10n.tr("settings.units.minutes")) {
                DebouncedNumberInput(
                    value: self.store.binding(\.checkinJitterMinutes, fallback: 0, touched: "checkin_jitter_minutes"),
                    range: 0 ... 30
                )
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
                DebouncedNumberInput(
                    value: self.store.binding(\.conversationAutoCloseMinutes, fallback: 10),
                    range: 1 ... 60
                )
            }

            SettingsRow(
                label: L10n.tr("settings.behavior.conversation.inactivity_timeout"),
                description: L10n.tr("settings.behavior.conversation.inactivity_timeout.description"),
                suffix: L10n.tr("settings.units.seconds")
            ) {
                DebouncedNumberInput(
                    value: self.store.binding(\.conversationInactivityTimeoutSeconds, fallback: 300),
                    range: 5 ... 600
                )
            }
        }
    }

    /// Goal-loop cadence. Lives under Behavior because it's a knob that
    /// shapes how often BoBe proactively re-evaluates goals — that's
    /// behavioural, not "advanced." The former Advanced panel collapsed
    /// into the panels its toggles actually belonged to.
    private var goalsSection: some View {
        CollapsibleSection(
            title: L10n.tr("settings.behavior.goals.title"),
            icon: "target",
            description: L10n.tr("settings.behavior.goals.description")
        ) {
            SettingsRow(
                label: L10n.tr("settings.behavior.goals.check_interval"),
                description: L10n.tr("settings.behavior.goals.check_interval.description"),
                suffix: L10n.tr("settings.units.seconds")
            ) {
                DebouncedNumberInput(
                    value: self.store.intBinding(\.goalCheckIntervalSeconds, fallback: 300),
                    range: 60 ... 7200
                )
            }
        }
    }

    private func addCheckinTime() {
        let formatted = Self.timeFormatter.string(from: self.newCheckinTime)
        guard let settings = self.store.settings,
              !settings.checkinTimes.contains(formatted)
        else { return }
        self.store.update(touched: "checkin_times") {
            $0.checkinTimes.append(formatted)
        }
    }

    private func removeCheckinTime(_ time: String) {
        self.store.update(touched: "checkin_times") {
            $0.checkinTimes.removeAll { $0 == time }
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
