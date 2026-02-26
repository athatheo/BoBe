import SwiftUI
import OSLog

private let logger = Logger(subsystem: "com.bobe.app", category: "PrivacyPanel")

/// Privacy settings panel — data size display and danger zone.
/// Based on PrivacySettings.tsx with beige data container and red danger zone.
struct PrivacyPanel: View {
    @State private var dataSize: DataSizeResponse?
    @State private var isLoading = false
    @Environment(\.theme) private var theme

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                // Local Storage section
                HStack(spacing: 8) {
                    Image(systemName: "externaldrive.fill")
                        .font(.system(size: 16))
                        .foregroundStyle(theme.colors.primary)
                    Text("Local Storage")
                        .font(.system(size: 16, weight: .semibold))
                        .foregroundStyle(theme.colors.text)
                }

                Text("All your data is stored locally on this Mac. Nothing is sent to the cloud.")
                    .font(.system(size: 13))
                    .foregroundStyle(theme.colors.textMuted)

                // Data size display
                VStack(alignment: .leading, spacing: 10) {
                    if let dataSize {
                        Text(formatBytes(dataSize.totalBytes))
                            .font(.system(size: 18, weight: .bold))
                            .foregroundStyle(theme.colors.text)

                        let entries = dataSizeEntries(dataSize.breakdown)
                            .filter { $0.1 > 1_048_576 }
                            .sorted { $0.1 > $1.1 }

                        ForEach(entries, id: \.0) { name, bytes in
                            HStack {
                                Text(name)
                                    .font(.system(size: 12))
                                    .foregroundStyle(theme.colors.textMuted)
                                Spacer()
                                Text(formatBytes(bytes))
                                    .font(.system(size: 12, weight: .medium))
                                    .foregroundStyle(theme.colors.text)
                            }
                        }
                    } else if isLoading {
                        Text("Calculating...")
                            .font(.system(size: 13))
                            .foregroundStyle(theme.colors.textMuted)
                    }
                }
                .padding(16)
                .background(
                    RoundedRectangle(cornerRadius: 10)
                        .fill(theme.colors.surface)
                        .stroke(theme.colors.border, lineWidth: 1)
                )

                Divider()

                // Danger Zone
                HStack(spacing: 8) {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .font(.system(size: 16))
                        .foregroundStyle(Color(hex: "C62828"))
                    Text("Danger Zone")
                        .font(.system(size: 16, weight: .semibold))
                        .foregroundStyle(Color(hex: "C62828"))
                }

                Text("Permanently delete all BoBe data from this Mac. This removes your database, downloaded models, and configuration. BoBe will quit and you'll need to set up again on next launch.")
                    .font(.system(size: 12))
                    .foregroundStyle(theme.colors.textMuted)

                Button {
                    deleteAllData()
                } label: {
                    HStack(spacing: 6) {
                        Image(systemName: "trash")
                        Text("Delete all data")
                    }
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(Color(hex: "C62828"))
                    .padding(.horizontal, 16)
                    .padding(.vertical, 8)
                    .background(
                        RoundedRectangle(cornerRadius: 8)
                            .fill(Color(hex: "FFEBEE"))
                            .stroke(Color(hex: "EF9A9A"), lineWidth: 1)
                    )
                }
                .buttonStyle(.plain)
            }
            .padding(24)
        }
        .task { await loadDataSize() }
    }

    private func dataSizeEntries(_ b: DataSizeBreakdown) -> [(String, Int)] {
        [("Models", b.models), ("Data", b.data), ("Logs", b.logs), ("Ollama", b.ollama)]
    }

    private func loadDataSize() async {
        isLoading = true
        defer { isLoading = false }
        do {
            dataSize = try await DaemonClient.shared.getDataSize()
        } catch {
            logger.warning("Failed to load data size: \(error.localizedDescription)")
        }
    }

    private func deleteAllData() {
        let alert = NSAlert()
        alert.messageText = "Delete All Data?"
        alert.informativeText = "This will permanently delete all BoBe data including memories, conversations, goals, and downloaded models. This action cannot be undone."
        alert.alertStyle = .critical
        alert.addButton(withTitle: "Delete")
        alert.addButton(withTitle: "Cancel")

        if alert.runModal() == .alertFirstButtonReturn {
            Task {
                do {
                    try await DaemonClient.shared.deleteAllData()
                    await MainActor.run {
                        NSApplication.shared.terminate(nil)
                    }
                } catch {
                    await MainActor.run {
                        showDeleteError(error.localizedDescription)
                    }
                }
            }
        }
    }

    @MainActor
    private func showDeleteError(_ message: String) {
        let alert = NSAlert()
        alert.messageText = "Delete failed"
        alert.informativeText = message
        alert.alertStyle = .warning
        alert.addButton(withTitle: "OK")
        alert.runModal()
    }
}

/// Goal Worker settings panel — autonomous execution config.
/// Based on GoalWorkerPanel with BobeToggle and DebouncedNumberInput.
struct GoalWorkerPanel: View {
    @State private var settings: DaemonSettings?
    @State private var isLoading = false
    @State private var error: String?
    @Environment(\.theme) private var theme

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
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
                    CollapsibleSection(
                        title: "Autonomous Execution",
                        icon: "play.circle.fill",
                        description: "Allow BoBe to work on goals independently"
                    ) {
                        SettingsRow(label: "Enable Goal Worker") {
                            BobeToggle(isOn: binding(\.goalWorkerEnabled))
                        }
                        SettingsRow(label: "Autonomous Mode", description: "Execute goals without asking permission") {
                            BobeToggle(isOn: binding(\.goalWorkerAutonomous))
                        }
                        SettingsRow(label: "Max concurrent", suffix: "tasks") {
                            DebouncedNumberInput(value: binding(\.goalWorkerMaxConcurrent), range: 1...10)
                        }
                    }

                    CollapsibleSection(
                        title: "Projects Directory",
                        icon: "folder.fill",
                        description: "Where BoBe creates project folders for goals"
                    ) {
                        HStack(spacing: 8) {
                            TextField("/path/to/projects", text: Binding(
                                get: { settings?.projectsDir ?? "" },
                                set: { settings?.projectsDir = $0 }
                            ))
                                .textFieldStyle(.roundedBorder)
                            Button("Browse...") { browseDirectory() }
                                .buttonStyle(.bordered)
                                .controlSize(.small)
                        }
                    }
                } else if isLoading {
                    HStack(spacing: 8) {
                        ProgressView().controlSize(.small)
                        Text("Loading goal worker settings...")
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
        Task {
            try? await Task.sleep(for: .seconds(0.6))
            guard let settings else { return }
            do {
                var req = SettingsUpdateRequest()
                req.goalWorkerEnabled = settings.goalWorkerEnabled
                req.goalWorkerAutonomous = settings.goalWorkerAutonomous
                req.goalWorkerMaxConcurrent = settings.goalWorkerMaxConcurrent
                req.projectsDir = settings.projectsDir
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
        if panel.runModal() == .OK, let url = panel.url {
            settings?.projectsDir = url.path
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
