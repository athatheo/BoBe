import SwiftUI

/// Advanced settings panel — similarity thresholds, intervals, MCP toggle.
struct AdvancedPanel: View {
    @State private var settings: DaemonSettings?
    @State private var isLoading = false
    @State private var error: String?
    @State private var saveTask: Task<Void, Never>?
    @Environment(\.theme) private var theme

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text("For Nerds")
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
                        Text("Loading daemon settings...")
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
            title: "Similarity Thresholds",
            icon: "square.3.layers.3d",
            description: "Vector similarity thresholds for memory operations"
        ) {
            SettingsRow(label: "Deduplication") {
                DebouncedDecimalInput(value: self.binding(\.similarityDeduplicationThreshold), range: 0 ... 1, step: 0.01)
            }
            SettingsRow(label: "Search recall") {
                DebouncedDecimalInput(value: self.binding(\.similaritySearchRecallThreshold), range: 0 ... 1, step: 0.01)
            }
            SettingsRow(label: "Clustering") {
                DebouncedDecimalInput(value: self.binding(\.similarityClusteringThreshold), range: 0 ... 1, step: 0.01)
            }
        }
    }

    private var goalsSection: some View {
        CollapsibleSection(
            title: "Goals",
            icon: "target",
            description: "Goal tracking intervals"
        ) {
            SettingsRow(label: "Check interval", description: "Seconds between goal relevance checks", suffix: "seconds") {
                DebouncedNumberInput(
                    value: Binding(
                        get: { Int(self.settings?.goalCheckIntervalSeconds ?? 300) },
                        set: { newVal in self.settings?.goalCheckIntervalSeconds = Double(newVal) }
                    ), range: 60 ... 7200
                )
            }
        }
    }

    private var learningSection: some View {
        CollapsibleSection(
            title: "Learning",
            icon: "brain.head.profile",
            description: "Background learning cycle timing"
        ) {
            SettingsRow(label: "Learning interval", description: "Minutes between learning cycles", suffix: "minutes") {
                DebouncedNumberInput(
                    value: self.binding(\.learningIntervalMinutes),
                    range: 1 ... 1440
                )
            }
        }
    }

    private var conversationSection: some View {
        CollapsibleSection(
            title: "Conversation",
            icon: "message.fill",
            description: "Advanced conversation timing"
        ) {
            SettingsRow(label: "Inactivity timeout", description: "Seconds before allowing new proactive reachout", suffix: "seconds") {
                DebouncedNumberInput(value: self.binding(\.conversationInactivityTimeoutSeconds), range: 5 ... 600)
            }
        }
    }

    private var projectsSection: some View {
        CollapsibleSection(
            title: "Projects",
            icon: "folder.fill",
            description: "Default directory where BoBe creates project folders from goals"
        ) {
            HStack(spacing: 8) {
                BobeTextField(placeholder: "/path/to/projects", text: self.binding(\.projectsDirectory))
                Button("Browse...") { self.browseDirectory() }
                    .bobeButton(.secondary, size: .small)
            }
        }
    }

    private var mcpSection: some View {
        CollapsibleSection(
            title: "MCP Protocol",
            icon: "server.rack",
            description: "Model Context Protocol server connections"
        ) {
            SettingsRow(label: "Enable MCP", description: "Connect to MCP servers for extended capabilities") {
                BobeToggle(isOn: self.binding(\.mcpEnabled))
            }
        }
    }

    // MARK: - Helpers

    private func binding<V>(_ keyPath: WritableKeyPath<DaemonSettings, V>) -> Binding<V> {
        Binding(
            get: {
                guard let settings else { fatalError("Binding accessed before settings loaded") }
                return settings[keyPath: keyPath]
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
                _ = try await DaemonClient.shared.updateSettings(req)
                self.error = nil
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
