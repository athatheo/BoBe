import SwiftUI

struct AdvancedPanel: View {
    @State private var settings: DaemonSettings?
    @State private var isLoading = false
    @State private var error: String?
    @State private var saveTask: Task<Void, Never>?
    @Environment(\.theme) private var theme

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text(L10n.tr("settings.advanced.title"))
                    .font(.title2.bold())
                    .foregroundStyle(self.theme.colors.text)

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
                    self.similaritySection
                    self.goalsSection
                    self.learningSection
                    self.conversationSection
                    self.projectsSection
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

    private var similaritySection: some View {
        CollapsibleSection(
            title: L10n.tr("settings.advanced.similarity.title"),
            icon: "square.3.layers.3d",
            description: L10n.tr("settings.advanced.similarity.description")
        ) {
            SettingsRow(label: L10n.tr("settings.advanced.similarity.deduplication")) {
                DebouncedDecimalInput(
                    value: self.binding(\.similarityDeduplicationThreshold, fallback: 0.85),
                    range: 0 ... 1,
                    step: 0.01
                )
            }
            SettingsRow(label: L10n.tr("settings.advanced.similarity.search_recall")) {
                DebouncedDecimalInput(
                    value: self.binding(\.similaritySearchRecallThreshold, fallback: 0.6),
                    range: 0 ... 1,
                    step: 0.01
                )
            }
            SettingsRow(label: L10n.tr("settings.advanced.similarity.clustering")) {
                DebouncedDecimalInput(
                    value: self.binding(\.similarityClusteringThreshold, fallback: 0.8),
                    range: 0 ... 1,
                    step: 0.01
                )
            }
        }
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

    private var learningSection: some View {
        CollapsibleSection(
            title: L10n.tr("settings.advanced.learning.title"),
            icon: "brain.head.profile",
            description: L10n.tr("settings.advanced.learning.description")
        ) {
            SettingsRow(
                label: L10n.tr("settings.advanced.learning.interval"),
                description: L10n.tr("settings.advanced.learning.interval.description"),
                suffix: L10n.tr("settings.units.minutes")
            ) {
                DebouncedNumberInput(
                    value: self.binding(\.learningIntervalMinutes, fallback: 15),
                    range: 1 ... 1440
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

    private var projectsSection: some View {
        CollapsibleSection(
            title: L10n.tr("settings.advanced.projects.title"),
            icon: "folder.fill",
            description: L10n.tr("settings.advanced.projects.description")
        ) {
            HStack(spacing: 8) {
                BobeTextField(
                    placeholder: L10n.tr("settings.advanced.projects.path_placeholder"),
                    text: self.binding(\.projectsDirectory, fallback: "")
                )
                Button(L10n.tr("settings.advanced.projects.action.browse")) { self.browseDirectory() }
                    .bobeButton(.secondary, size: .small)
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
                self.debounceSave()
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
                self.debounceSave()
            }
        )
    }

    private func debounceSave() {
        self.saveTask?.cancel()
        self.saveTask = Task {
            try? await Task.sleep(for: .seconds(0.6))
            guard !Task.isCancelled, let settings else { return }
            do {
                var req = SettingsUpdateRequest()
                req.similarityDeduplicationThreshold = settings.similarityDeduplicationThreshold
                req.similaritySearchRecallThreshold = settings.similaritySearchRecallThreshold
                req.similarityClusteringThreshold = settings.similarityClusteringThreshold
                req.goalCheckIntervalSeconds = settings.goalCheckIntervalSeconds
                req.learningIntervalMinutes = settings.learningIntervalMinutes
                req.conversationInactivityTimeoutSeconds = settings.conversationInactivityTimeoutSeconds
                req.projectsDirectory = settings.projectsDirectory
                req.mcpEnabled = settings.mcpEnabled
                req.localeOverride = settings.localeOverride
                _ = try await DaemonClient.shared.updateSettings(req)
                self.error = nil

                // Refresh to pick up the daemon's effective_locale
                let refreshed = try await DaemonClient.shared.getSettings()
                self.settings = refreshed
                BobeStore.shared.effectiveLocale = refreshed.effectiveLocale
                BobeStore.shared.supportedLocales = refreshed.supportedLocales
            } catch {
                self.error = error.localizedDescription
            }
        }
    }

    private func browseDirectory() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        if panel.runModal() == .OK, let url = panel.url {
            self.settings?.projectsDirectory = url.path
            self.debounceSave()
        }
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
