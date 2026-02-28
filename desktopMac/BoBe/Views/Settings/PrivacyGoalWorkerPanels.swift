import SwiftUI

/// Privacy settings panel — local-only data posture.
struct PrivacyPanel: View {
    @State private var showDeleteControls = false
    @State private var deleteConfirmationText = ""
    @State private var isDeletingAll = false
    @State private var statusMessage: String?
    @State private var statusIsError = false
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

                VStack(alignment: .leading, spacing: 10) {
                    HStack(spacing: 8) {
                        Image(systemName: "trash.fill")
                            .font(.system(size: 15))
                            .foregroundStyle(theme.colors.primary)
                        Text("Danger Zone")
                            .font(.system(size: 14, weight: .semibold))
                            .foregroundStyle(theme.colors.text)
                    }

                    Text("Delete all goals, memories, non-default souls, non-default user profiles, and MCP server configs from this device.")
                        .font(.system(size: 12))
                        .foregroundStyle(theme.colors.textMuted)

                    if showDeleteControls {
                        VStack(alignment: .leading, spacing: 8) {
                            Text("Type DELETE to confirm")
                                .font(.system(size: 11, weight: .medium))
                                .foregroundStyle(theme.colors.textMuted)
                            BobeTextField(placeholder: "DELETE", text: $deleteConfirmationText, width: 220)

                            HStack(spacing: 8) {
                                Button("Cancel") {
                                    showDeleteControls = false
                                    deleteConfirmationText = ""
                                }
                                .bobeButton(.secondary, size: .small)

                                Button(isDeletingAll ? "Deleting..." : "Delete All Data") {
                                    Task { await deleteAllData() }
                                }
                                .bobeButton(.primary, size: .small)
                                .disabled(deleteConfirmationText != "DELETE" || isDeletingAll)

                                if isDeletingAll {
                                    BobeSpinner(size: 14)
                                }
                            }
                        }
                    } else {
                        Button("Delete All Data") {
                            statusMessage = nil
                            showDeleteControls = true
                        }
                        .bobeButton(.primary, size: .small)
                    }

                    if let statusMessage {
                        Text(statusMessage)
                            .font(.system(size: 11))
                            .foregroundStyle(statusIsError ? theme.colors.primary : theme.colors.secondary)
                    }
                }
                .padding(16)
                .background(
                    RoundedRectangle(cornerRadius: 10)
                        .fill(theme.colors.surface)
                        .stroke(theme.colors.primary.opacity(0.25), lineWidth: 1)
                )
            }
            .padding(24)
        }
    }

    private func deleteAllData() async {
        guard deleteConfirmationText == "DELETE" else { return }
        isDeletingAll = true
        defer { isDeletingAll = false }

        var errors: [String] = []

        do {
            let goals = try await DaemonClient.shared.listGoals().goals
            for goal in goals {
                do {
                    _ = try await DaemonClient.shared.deleteGoal(goal.id)
                } catch {
                    errors.append("goal \(goal.id)")
                }
            }
        } catch {
            errors.append("goals")
        }

        do {
            let memories = try await DaemonClient.shared.listMemories().memories
            for memory in memories {
                do {
                    _ = try await DaemonClient.shared.deleteMemory(memory.id)
                } catch {
                    errors.append("memory \(memory.id)")
                }
            }
        } catch {
            errors.append("memories")
        }

        do {
            let souls = try await DaemonClient.shared.listSouls().souls.filter { !$0.isDefault }
            for soul in souls {
                do {
                    _ = try await DaemonClient.shared.deleteSoul(soul.id)
                } catch {
                    errors.append("soul \(soul.name)")
                }
            }
        } catch {
            errors.append("souls")
        }

        do {
            let profiles = try await DaemonClient.shared.listUserProfiles().profiles
                .filter { !$0.isDefault }
            for profile in profiles {
                do {
                    _ = try await DaemonClient.shared.deleteUserProfile(profile.id)
                } catch {
                    errors.append("profile \(profile.name)")
                }
            }
        } catch {
            errors.append("profiles")
        }

        do {
            let servers = try await DaemonClient.shared.listMCPServers().servers
            for server in servers {
                do {
                    try await DaemonClient.shared.deleteMCPServer(server.name)
                } catch {
                    errors.append("mcp \(server.name)")
                }
            }
        } catch {
            errors.append("mcp")
        }

        showDeleteControls = false
        deleteConfirmationText = ""
        if errors.isEmpty {
            statusIsError = false
            statusMessage = "All local data was deleted."
        } else {
            statusIsError = true
            let details = errors.prefix(4).joined(separator: ", ")
            let suffix = errors.count > 4 ? ", ..." : ""
            statusMessage = "Delete completed with issues: \(details)\(suffix)"
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
                        Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(theme.colors.primary)
                        Text(error).font(.system(size: 12)).foregroundStyle(theme.colors.primary)
                    }
                    .padding(10)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(RoundedRectangle(cornerRadius: 8).fill(theme.colors.primary.opacity(0.08)))
                }

                if isLoading && settings == nil {
                    BobeSpinner(size: 16)
                        .frame(maxWidth: .infinity, alignment: .center)
                        .padding(.top, 40)
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

                    Text("The goal worker autonomously breaks down goals into steps and executes them "
                        + "using tools. Goals that fail repeatedly are paused automatically.")
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

// MARK: - Previews

#Preview("Privacy Panel") {
    PrivacyPanel()
        .environment(\.theme, allThemes[0])
        .frame(width: 600, height: 500)
}
