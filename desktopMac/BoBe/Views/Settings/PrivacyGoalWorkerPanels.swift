import SwiftUI

/// Privacy settings panel — local-only data posture.
struct PrivacyPanel: View {
    @Environment(\.theme) private var theme

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                HStack(spacing: 8) {
                    Image(systemName: "externaldrive.fill")
                        .font(.system(size: 16))
                        .foregroundStyle(theme.colors.primary)
                    Text("Local Storage")
                        .font(.system(size: 16, weight: .semibold))
                        .foregroundStyle(theme.colors.text)
                }

                Text("BoBe stores data locally on this Mac. Souls, goals, memories, user profile, and runtime settings remain on-device.")
                    .font(.system(size: 13))
                    .foregroundStyle(theme.colors.textMuted)

                VStack(alignment: .leading, spacing: 8) {
                    Text("Included in local state")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(theme.colors.text)
                    Text("• Souls and goals\n• Memories and observations\n• User profile and settings\n• Local model metadata")
                        .font(.system(size: 12))
                        .foregroundStyle(theme.colors.textMuted)
                }
                .padding(16)
                .background(
                    RoundedRectangle(cornerRadius: 10)
                        .fill(theme.colors.surface)
                        .stroke(theme.colors.border, lineWidth: 1)
                )
            }
            .padding(24)
        }
    }
}

/// Goal Worker settings panel — autonomous goal execution configuration.
struct GoalWorkerPanel: View {
    @State private var settings: DaemonSettings?
    @State private var workerStatus: GoalWorkerStatusResponse?
    @State private var isLoading = false
    @State private var error: String?
    @State private var saveTask: Task<Void, Never>?
    @Environment(\.theme) private var theme

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text("Goal Worker")
                    .font(.title2.bold())
                    .foregroundStyle(theme.colors.text)

                if let error {
                    HStack(spacing: 6) {
                        Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(.red)
                        Text(error).font(.system(size: 12)).foregroundStyle(.red)
                    }
                    .padding(10)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(RoundedRectangle(cornerRadius: 8).fill(.red.opacity(0.08)))
                }

                if isLoading && settings == nil {
                    ProgressView().frame(maxWidth: .infinity, alignment: .center).padding(.top, 40)
                } else if let settings {
                    CollapsibleSection(title: "Autonomous Execution", icon: "gearshape.2.fill") {
                        VStack(alignment: .leading, spacing: 12) {
                            HStack {
                                Text("Enabled").font(.system(size: 13)).foregroundStyle(theme.colors.text)
                                Spacer()
                                BobeToggle(isOn: Binding(
                                    get: { settings.goalWorkerEnabled },
                                    set: { val in self.settings?.goalWorkerEnabled = val; debounceSave() }
                                ))
                            }

                            HStack {
                                Text("Autonomous mode").font(.system(size: 13)).foregroundStyle(theme.colors.text)
                                Spacer()
                                BobeToggle(isOn: Binding(
                                    get: { settings.goalWorkerAutonomous },
                                    set: { val in self.settings?.goalWorkerAutonomous = val; debounceSave() }
                                ))
                            }

                            HStack {
                                Text("Max concurrent").font(.system(size: 13)).foregroundStyle(theme.colors.text)
                                Spacer()
                                DebouncedNumberInput(value: Binding(
                                    get: { settings.goalWorkerMaxConcurrent },
                                    set: { val in self.settings?.goalWorkerMaxConcurrent = val; debounceSave() }
                                ), range: 1...10)
                            }
                        }
                    }

                    if let status = workerStatus {
                        CollapsibleSection(title: "Status", icon: "chart.bar.fill") {
                            VStack(alignment: .leading, spacing: 8) {
                                statusRow("Active goals", "\(status.activeGoalsCount)")
                                statusRow("Pending approval", "\(status.pendingApprovalCount)")
                                statusRow("Worker enabled", status.enabled ? "Yes" : "No")
                            }
                        }
                    }

                    Text("The goal worker autonomously breaks down goals into steps and executes them using tools. Goals that fail repeatedly are paused automatically.")
                        .font(.system(size: 11))
                        .foregroundStyle(theme.colors.textMuted)
                }
            }
            .padding(24)
        }
        .task { await loadSettings() }
    }

    private func statusRow(_ label: String, _ value: String) -> some View {
        HStack {
            Text(label).font(.system(size: 12)).foregroundStyle(theme.colors.textMuted)
            Spacer()
            Text(value).font(.system(size: 12, weight: .medium)).foregroundStyle(theme.colors.text)
        }
    }

    private func loadSettings() async {
        isLoading = true
        defer { isLoading = false }
        do {
            settings = try await DaemonClient.shared.getSettings()
            workerStatus = try await DaemonClient.shared.goalWorkerStatus()
            error = nil
        } catch {
            self.error = error.localizedDescription
        }
    }

    private func debounceSave() {
        saveTask?.cancel()
        saveTask = Task {
            try? await Task.sleep(for: .seconds(0.6))
            guard !Task.isCancelled, let settings else { return }
            do {
                var req = SettingsUpdateRequest()
                req.goalWorkerEnabled = settings.goalWorkerEnabled
                req.goalWorkerAutonomous = settings.goalWorkerAutonomous
                req.goalWorkerMaxConcurrent = settings.goalWorkerMaxConcurrent
                _ = try await DaemonClient.shared.updateSettings(req)
                self.error = nil
            } catch {
                self.error = error.localizedDescription
            }
        }
    }
}
