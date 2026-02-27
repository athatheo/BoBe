import SwiftUI

/// Advanced settings panel ("For Nerds") — similarity thresholds, intervals, MCP toggle.
/// Based on AdvancedSettings.tsx with DebouncedDecimalInput and description text.
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
                    .foregroundStyle(theme.colors.text)

                if let error {
                    HStack(spacing: 6) {
                        Image(systemName: "exclamationmark.triangle.fill")
                            .foregroundStyle(.red)
                        Text(error)
                            .font(.system(size: 12))
                            .foregroundStyle(.red)
                    }
                    .padding(10)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(RoundedRectangle(cornerRadius: 8).fill(.red.opacity(0.08)))
                }

                if let _ = settings {
                    similaritySection
                    goalsSection
                    learningSection
                    conversationSection
                    projectsSection
                    mcpSection
                } else if isLoading {
                    HStack(spacing: 8) {
                        ProgressView().controlSize(.small)
                        Text("Loading daemon settings...")
                            .font(.system(size: 13))
                            .foregroundStyle(theme.colors.textMuted)
                    }
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.top, 40)
                }
            }
            .padding(24)
        }
        .task { await loadSettings() }
    }

    private var similaritySection: some View {
        CollapsibleSection(
            title: "Similarity Thresholds",
            icon: "square.3.layers.3d",
            description: "Vector similarity thresholds for memory operations"
        ) {
            SettingsRow(label: "Deduplication") {
                DebouncedDecimalInput(value: binding(\.similarityDeduplicationThreshold), range: 0...1, step: 0.01)
            }
            SettingsRow(label: "Search recall") {
                DebouncedDecimalInput(value: binding(\.similaritySearchRecallThreshold), range: 0...1, step: 0.01)
            }
            SettingsRow(label: "Clustering") {
                DebouncedDecimalInput(value: binding(\.similarityClusteringThreshold), range: 0...1, step: 0.01)
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
                DebouncedNumberInput(value: Binding(
                    get: { Int(settings?.goalCheckIntervalSeconds ?? 300) },
                    set: { newVal in settings?.goalCheckIntervalSeconds = Double(newVal) }
                ), range: 60...7200)
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
                    value: binding(\.learningIntervalMinutes),
                    range: 1...1440
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
                DebouncedNumberInput(value: binding(\.conversationInactivityTimeoutSeconds), range: 5...600)
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
                TextField("/path/to/projects", text: binding(\.projectsDirectory))
                    .textFieldStyle(.roundedBorder)
                Button("Browse...") { browseDirectory() }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
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
                BobeToggle(isOn: binding(\.mcpEnabled))
            }
        }
    }

    // MARK: - Helpers

    private func binding<V>(_ keyPath: WritableKeyPath<DaemonSettings, V>) -> Binding<V> {
        Binding(
            get: { settings![keyPath: keyPath] },
            set: { newValue in
                settings![keyPath: keyPath] = newValue
                debounceSave()
            }
        )
    }

    private func debounceSave() {
        saveTask?.cancel()
        saveTask = Task {
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
            settings?.projectsDirectory = url.path
            debounceSave()
        }
    }

    private func loadSettings() async {
        isLoading = true
        defer { isLoading = false }
        do {
            settings = try await DaemonClient.shared.getSettings()
        } catch {
            self.error = error.localizedDescription
        }
    }
}
