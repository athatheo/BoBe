import SwiftUI

struct AdvancedPanel: View {
    @State private var settings: DaemonSettings?
    @State private var isLoading = false
    @State private var error: String?
    @State private var savedMessage: String?
    @State private var debouncer = SettingsDebouncer()
    @State private var restartFields: Set<String> = []
    @State private var bannerDismissed = false
    @Environment(\.theme) private var theme

    /// Daemon claims hot-apply but MCP map is captured at boot — force banner locally.
    private static let deferToRestartFields: Set<String> = [
        "mcp_enabled",
    ]

    private var visibleRestartFields: Set<String> {
        self.bannerDismissed ? [] : self.restartFields
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text(L10n.tr("settings.advanced.title"))
                    .font(.title2.bold())
                    .foregroundStyle(self.theme.colors.text)

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
                    self.goalsSection
                    self.conversationSection
                    self.mcpSection
                } else if self.isLoading {
                    HStack(spacing: 8) {
                        BobeSpinner(size: 14)
                        Text(L10n.tr("settings.advanced.loading"))
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

    private var goalsSection: some View {
        CollapsibleSection(
            title: L10n.tr("settings.advanced.goals.title"),
            icon: "target",
            description: L10n.tr("settings.advanced.goals.description")
        ) {
            SettingsRow(
                label: L10n.tr("settings.advanced.goals.check_interval"),
                description: L10n.tr("settings.advanced.goals.check_interval.description"),
                suffix: L10n.tr("settings.units.seconds")
            ) {
                DebouncedNumberInput(
                    value: self.intBinding(\.goalCheckIntervalSeconds, fallback: 300),
                    range: 60 ... 7200
                )
            }
        }
    }

    private var conversationSection: some View {
        CollapsibleSection(
            title: L10n.tr("settings.advanced.conversation.title"),
            icon: "message.fill",
            description: L10n.tr("settings.advanced.conversation.description")
        ) {
            SettingsRow(
                label: L10n.tr("settings.advanced.conversation.inactivity_timeout"),
                description: L10n.tr("settings.advanced.conversation.inactivity_timeout.description"),
                suffix: L10n.tr("settings.units.seconds")
            ) {
                DebouncedNumberInput(
                    value: self.binding(\.conversationInactivityTimeoutSeconds, fallback: 300),
                    range: 5 ... 600
                )
            }
        }
    }

    private var mcpSection: some View {
        CollapsibleSection(
            title: L10n.tr("settings.advanced.mcp.title"),
            icon: "server.rack",
            description: L10n.tr("settings.advanced.mcp.description")
        ) {
            SettingsRow(
                label: L10n.tr("settings.advanced.mcp.enable"),
                description: L10n.tr("settings.advanced.mcp.enable.description")
            ) {
                BobeToggle(isOn: self.binding(\.mcpEnabled, fallback: false))
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

    private func intBinding(
        _ keyPath: WritableKeyPath<DaemonSettings, Double>,
        fallback: @autoclosure @escaping () -> Int
    ) -> Binding<Int> {
        Binding(
            get: {
                Int((self.settings?[keyPath: keyPath] ?? Double(fallback())).rounded())
            },
            set: { newValue in
                guard var current = self.settings else { return }
                current[keyPath: keyPath] = Double(newValue)
                self.settings = current
                self.debounceSave(touched: Self.fieldKey(for: keyPath))
            }
        )
    }

    private static func fieldKey<V>(for keyPath: WritableKeyPath<DaemonSettings, V>) -> String? {
        switch keyPath {
        case \DaemonSettings.goalCheckIntervalSeconds: "goal_check_interval_seconds"
        case \DaemonSettings.conversationInactivityTimeoutSeconds: "conversation_inactivity_timeout_seconds"
        case \DaemonSettings.mcpEnabled: "mcp_enabled"
        default: nil
        }
    }

    private func debounceSave(touched: String? = nil) {
        let touchedFields: Set<String> = touched.map { [$0] } ?? []
        self.debouncer.debounce {
            guard let settings else { return }
            do {
                var req = SettingsUpdateRequest()
                req.goalCheckIntervalSeconds = settings.goalCheckIntervalSeconds
                req.conversationInactivityTimeoutSeconds = settings.conversationInactivityTimeoutSeconds
                req.mcpEnabled = settings.mcpEnabled
                let resp = try await DaemonClient.shared.updateSettings(req)
                self.applySaveResponse(touched: touchedFields, response: resp)
            } catch {
                self.error = error.localizedDescription
                self.savedMessage = nil
            }
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

    private func applyRestartFields(touched: Set<String>, response: SettingsUpdateResponse) {
        let shadowed = touched.intersection(Self.deferToRestartFields)
        let daemonReported = Set(response.restartRequiredFields)
        let combined = shadowed.union(daemonReported)
        if combined.isEmpty { return }
        self.restartFields = self.restartFields.union(combined)
        self.bannerDismissed = false
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
