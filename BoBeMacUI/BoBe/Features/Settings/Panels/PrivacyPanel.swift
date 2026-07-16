import SwiftUI

struct PrivacyPanel: View {
    private struct DataFlowPresentation {
        let icon: String
        let title: String
        let body: String
        let color: Color
    }

    private let deleteKeyword = "DELETE"

    @State private var showDeleteControls = false
    @State private var deleteConfirmationText = ""
    @State private var isDeletingAll = false
    @State private var statusMessage: String?
    @State private var statusIsError = false
    @Environment(SettingsStore.self) private var settingsStore
    @Environment(\.theme) private var theme

    private var engineDataFlow: DataFlowPresentation {
        switch self.settingsStore.settings?.engine {
        case EngineKind.copilotCloud:
            DataFlowPresentation(
                icon: "cloud.fill",
                title: L10n.tr("settings.privacy.data_flow.cloud.title"),
                body: L10n.tr("settings.privacy.data_flow.cloud.body"),
                color: self.theme.colors.warning
            )
        case EngineKind.local:
            DataFlowPresentation(
                icon: "desktopcomputer",
                title: L10n.tr("settings.privacy.data_flow.local_engine.title"),
                body: L10n.tr("settings.privacy.data_flow.local_engine.body"),
                color: self.theme.colors.success
            )
        default:
            DataFlowPresentation(
                icon: "questionmark.circle",
                title: L10n.tr("settings.privacy.data_flow.unknown_engine.title"),
                body: L10n.tr("settings.privacy.data_flow.unknown_engine.body"),
                color: self.theme.colors.textMuted
            )
        }
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                VStack(alignment: .leading, spacing: 12) {
                    HStack(spacing: 8) {
                        Image(systemName: "arrow.left.arrow.right")
                            .font(.system(size: 16))
                            .foregroundStyle(self.theme.colors.primary)
                        Text(L10n.tr("settings.privacy.data_flow.title"))
                            .bobeTextStyle(.heading)
                            .foregroundStyle(self.theme.colors.text)
                    }

                    Text(L10n.tr("settings.privacy.data_flow.description"))
                        .bobeTextStyle(.settingsBody)
                        .foregroundStyle(self.theme.colors.textMuted)

                    self.dataFlowRow(
                        icon: "internaldrive.fill",
                        title: L10n.tr("settings.privacy.data_flow.local.title"),
                        body: L10n.tr("settings.privacy.data_flow.local.body"),
                        color: self.theme.colors.success
                    )
                    self.dataFlowRow(
                        icon: self.engineDataFlow.icon,
                        title: self.engineDataFlow.title,
                        body: self.engineDataFlow.body,
                        color: self.engineDataFlow.color
                    )
                    self.dataFlowRow(
                        icon: "camera.viewfinder",
                        title: L10n.tr("settings.privacy.data_flow.capture.title"),
                        body: L10n.tr("settings.privacy.data_flow.capture.body"),
                        color: self.theme.colors.warning
                    )
                    self.dataFlowRow(
                        icon: "wrench.and.screwdriver.fill",
                        title: L10n.tr("settings.privacy.data_flow.tools.title"),
                        body: L10n.tr("settings.privacy.data_flow.tools.body"),
                        color: self.theme.colors.tertiary
                    )
                }

                Divider()

                HStack(spacing: 8) {
                    Image(systemName: "externaldrive.fill")
                        .font(.system(size: 16))
                        .foregroundStyle(self.theme.colors.primary)
                    Text(L10n.tr("settings.privacy.storage.title"))
                        .bobeTextStyle(.heading)
                        .foregroundStyle(self.theme.colors.text)
                }

                Text(L10n.tr("settings.privacy.storage.description"))
                    .bobeTextStyle(.settingsBody)
                    .foregroundStyle(self.theme.colors.textMuted)

                VStack(alignment: .leading, spacing: 8) {
                    Text(L10n.tr("settings.privacy.storage.included.title"))
                        .bobeTextStyle(.rowTitle)
                        .foregroundStyle(self.theme.colors.text)
                    Text(L10n.tr("settings.privacy.storage.included.list"))
                        .bobeTextStyle(.helper)
                        .foregroundStyle(self.theme.colors.textMuted)
                }
                .padding(16)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(
                    RoundedRectangle(cornerRadius: 10)
                        .fill(self.theme.colors.surface)
                        .stroke(self.theme.colors.border, lineWidth: 1)
                )

                VStack(alignment: .leading, spacing: 10) {
                    HStack(spacing: 8) {
                        Image(systemName: "trash.fill")
                            .font(.system(size: 15))
                            .foregroundStyle(self.theme.colors.destructive)
                        Text(L10n.tr("settings.privacy.danger.title"))
                            .bobeTextStyle(.rowTitle)
                            .foregroundStyle(self.theme.colors.text)
                    }

                    Text(L10n.tr("settings.privacy.danger.description"))
                        .bobeTextStyle(.helper)
                        .foregroundStyle(self.theme.colors.textMuted)

                    if self.showDeleteControls {
                        VStack(alignment: .leading, spacing: 8) {
                            Text(L10n.tr("settings.privacy.danger.confirm_prompt_format", self.deleteKeyword))
                                .bobeTextStyle(.helper)
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
                                .bobeButton(.destructive, size: .small)
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
                        .bobeButton(.destructive, size: .small)
                    }

                    if let statusMessage {
                        Text(statusMessage)
                            .bobeTextStyle(.helper)
                            .foregroundStyle(self.statusIsError ? self.theme.colors.error : self.theme.colors.success)
                    }
                }
                .padding(16)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(
                    RoundedRectangle(cornerRadius: 10)
                        .fill(self.theme.colors.surface)
                        .stroke(self.theme.colors.destructive.opacity(0.35), lineWidth: 1)
                )
            }
            .padding(24)
        }
        .task { await self.settingsStore.loadIfNeeded() }
    }

    private func dataFlowRow(
        icon: String,
        title: String,
        body: String,
        color: Color
    ) -> some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: icon)
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(color)
                .frame(width: 18)
                .padding(.top, 2)
            VStack(alignment: .leading, spacing: 3) {
                Text(title)
                    .bobeTextStyle(.rowTitle)
                    .foregroundStyle(self.theme.colors.text)
                Text(body)
                    .bobeTextStyle(.helper)
                    .foregroundStyle(self.theme.colors.textMuted)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 10)
                .fill(self.theme.colors.surface)
                .stroke(self.theme.colors.border, lineWidth: 1)
        )
    }

    private func deleteAllData() async {
        guard self.deleteConfirmationText == self.deleteKeyword else { return }
        self.isDeletingAll = true
        defer { isDeletingAll = false }

        do {
            let response = try await DaemonClient.shared.purgePersonalData()
            self.showDeleteControls = false
            self.deleteConfirmationText = ""
            self.statusIsError = false
            self.statusMessage = response.message
        } catch {
            self.statusIsError = true
            self.statusMessage = error.localizedDescription
        }
    }
}

#if !SPM_BUILD
    #Preview("Privacy Panel") {
        PrivacyPanel()
            .environment(\.theme, allThemes[0])
            .environment(SettingsStore.shared)
            .frame(width: 600, height: 500)
    }
#endif
