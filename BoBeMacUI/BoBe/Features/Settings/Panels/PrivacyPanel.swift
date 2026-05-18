import SwiftUI

struct PrivacyPanel: View {
    private let deleteKeyword = "DELETE"

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
                        .foregroundStyle(self.theme.colors.primary)
                    Text(L10n.tr("settings.privacy.storage.title"))
                        .font(.system(size: 16, weight: .semibold))
                        .foregroundStyle(self.theme.colors.text)
                }

                Text(L10n.tr("settings.privacy.storage.description"))
                    .font(.system(size: 13))
                    .foregroundStyle(self.theme.colors.textMuted)

                VStack(alignment: .leading, spacing: 8) {
                    Text(L10n.tr("settings.privacy.storage.included.title"))
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(self.theme.colors.text)
                    Text(L10n.tr("settings.privacy.storage.included.list"))
                        .font(.system(size: 12))
                        .foregroundStyle(self.theme.colors.textMuted)
                }
                .padding(16)
                .background(
                    RoundedRectangle(cornerRadius: 10)
                        .fill(self.theme.colors.surface)
                        .stroke(self.theme.colors.border, lineWidth: 1)
                )

                VStack(alignment: .leading, spacing: 10) {
                    HStack(spacing: 8) {
                        Image(systemName: "trash.fill")
                            .font(.system(size: 15))
                            .foregroundStyle(self.theme.colors.primary)
                        Text(L10n.tr("settings.privacy.danger.title"))
                            .font(.system(size: 14, weight: .semibold))
                            .foregroundStyle(self.theme.colors.text)
                    }

                    Text(L10n.tr("settings.privacy.danger.description"))
                        .font(.system(size: 12))
                        .foregroundStyle(self.theme.colors.textMuted)

                    if self.showDeleteControls {
                        VStack(alignment: .leading, spacing: 8) {
                            Text(L10n.tr("settings.privacy.danger.confirm_prompt_format", self.deleteKeyword))
                                .font(.system(size: 11, weight: .medium))
                                .foregroundStyle(self.theme.colors.textMuted)
                            BobeTextField(placeholder: self.deleteKeyword, text: self.$deleteConfirmationText, width: 220)

                            HStack(spacing: 8) {
                                Button(L10n.tr("settings.editor.action.cancel")) {
                                    self.showDeleteControls = false
                                    self.deleteConfirmationText = ""
                                }
                                .bobeButton(.secondary, size: .small)

                                Button(
                                    self.isDeletingAll
                                        ? L10n.tr("settings.privacy.danger.action.deleting")
                                        : L10n.tr("settings.privacy.danger.action.delete_all")
                                ) {
                                    Task { await self.deleteAllData() }
                                }
                                .bobeButton(.primary, size: .small)
                                .disabled(self.deleteConfirmationText != self.deleteKeyword || self.isDeletingAll)

                                if self.isDeletingAll {
                                    BobeSpinner(size: 14)
                                }
                            }
                        }
                    } else {
                        Button(L10n.tr("settings.privacy.danger.action.delete_all")) {
                            statusMessage = nil
                            self.showDeleteControls = true
                        }
                        .bobeButton(.primary, size: .small)
                    }

                    if let statusMessage {
                        Text(statusMessage)
                            .font(.system(size: 11))
                            .foregroundStyle(self.statusIsError ? self.theme.colors.primary : self.theme.colors.secondary)
                    }
                }
                .padding(16)
                .background(
                    RoundedRectangle(cornerRadius: 10)
                        .fill(self.theme.colors.surface)
                        .stroke(self.theme.colors.primary.opacity(0.25), lineWidth: 1)
                )
            }
            .padding(24)
        }
    }

    private func deleteAllData() async {
        guard self.deleteConfirmationText == self.deleteKeyword else { return }
        self.isDeletingAll = true
        defer { isDeletingAll = false }

        var errors: [String] = []

        do {
            let goals = try await DaemonClient.shared.listGoals(includeArchived: true).goals
            for goal in goals {
                do {
                    try await DaemonClient.shared.deleteGoal(goal.id)
                } catch {
                    errors.append(L10n.tr("settings.privacy.danger.error.goal_format", goal.id))
                }
            }
        } catch {
            errors.append(L10n.tr("settings.privacy.danger.error.goals"))
        }

        // Memory is one document; reset to skeleton — no per-row delete post-pivot.
        do {
            _ = try await DaemonClient.shared.updateMemory(memoryDefaultBody)
        } catch {
            errors.append(L10n.tr("settings.privacy.danger.error.memory"))
        }

        do {
            let souls = try await DaemonClient.shared.listSouls().souls.filter { !$0.isDefault }
            for soul in souls {
                do {
                    try await DaemonClient.shared.deleteSoul(soul.id)
                } catch {
                    errors.append(L10n.tr("settings.privacy.danger.error.soul_format", soul.name))
                }
            }
        } catch {
            errors.append(L10n.tr("settings.privacy.danger.error.souls"))
        }

        do {
            let profiles = try await DaemonClient.shared.listUserProfiles().profiles
                .filter { !$0.isDefault }
            for profile in profiles {
                do {
                    try await DaemonClient.shared.deleteUserProfile(profile.id)
                } catch {
                    errors.append(L10n.tr("settings.privacy.danger.error.profile_format", profile.name))
                }
            }
        } catch {
            errors.append(L10n.tr("settings.privacy.danger.error.profiles"))
        }

        do {
            _ = try await DaemonClient.shared.resetMCPConfig()
        } catch {
            errors.append(L10n.tr("settings.privacy.danger.error.mcp"))
        }

        self.showDeleteControls = false
        self.deleteConfirmationText = ""
        if errors.isEmpty {
            self.statusIsError = false
            self.statusMessage = L10n.tr("settings.privacy.danger.status.success")
        } else {
            self.statusIsError = true
            let details = errors.prefix(4).joined(separator: ", ")
            let suffix = errors.count > 4 ? L10n.tr("settings.privacy.danger.status.more_suffix") : ""
            self.statusMessage = L10n.tr("settings.privacy.danger.status.partial_format", details, suffix)
        }
    }
}

#if !SPM_BUILD
    #Preview("Privacy Panel") {
        PrivacyPanel()
            .environment(\.theme, allThemes[0])
            .frame(width: 600, height: 500)
    }
#endif
