import SwiftUI

struct DoneStepView: View {
    let engineChoice: EngineChoice?
    let preferredName: String
    let firstGoal: String
    let proactivityLevel: ProactivityLevel
    let onLaunch: () -> Void
    @Environment(\.theme) private var theme

    @State private var settingsApplied = false
    @State private var settingsError: String?

    var body: some View {
        VStack(spacing: 24) {
            Spacer()

            ZStack {
                Circle()
                    .fill(self.theme.colors.secondary.opacity(0.18))
                    .frame(width: 88, height: 88)
                Image(systemName: "checkmark.circle.fill")
                    .font(.system(size: 44, weight: .light))
                    .foregroundStyle(self.theme.colors.secondary)
            }

            VStack(spacing: 12) {
                Text(L10n.tr("setup.done.title"))
                    .bobeTextStyle(.setupTitle)
                    .foregroundStyle(self.theme.colors.text)
                Text(L10n.tr("setup.done.body"))
                    .bobeTextStyle(.setupBody)
                    .foregroundStyle(self.theme.colors.textMuted)
                    .multilineTextAlignment(.center)
                    .lineSpacing(4)
                if !self.firstGoal.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                    Label(
                        L10n.tr("setup.done.first_goal_format", self.firstGoal),
                        systemImage: "target"
                    )
                    .bobeTextStyle(.helper)
                    .foregroundStyle(self.theme.colors.text)
                    .multilineTextAlignment(.center)
                }
                if let error = self.settingsError {
                    VStack(spacing: 6) {
                        Text(error)
                            .bobeTextStyle(.helper)
                            .foregroundStyle(self.theme.colors.primary)
                            .multilineTextAlignment(.center)
                        Button(L10n.tr("app.common.retry"), action: self.retrySettings)
                            .bobeButton(.secondary, size: .small)
                    }
                }
            }

            Spacer()

            Button(L10n.tr("setup.done.launch"), action: self.onLaunch)
                .bobeButton(.primary, size: .regular)
                .keyboardShortcut(.defaultAction)
                .disabled(!self.settingsApplied)

            Text(L10n.tr("setup.done.hint"))
                .bobeTextStyle(.helper)
                .foregroundStyle(self.theme.colors.textMuted)
                .multilineTextAlignment(.center)
        }
        .task {
            await self.applyEngineSettings()
        }
    }

    private func applyEngineSettings() async {
        let behavior = self.proactivityLevel.settings
        let request = switch self.engineChoice {
        case .local:
            SettingsUpdateRequest(
                captureEnabled: behavior.captureEnabled,
                captureIntervalSeconds: behavior.captureIntervalSeconds,
                checkinEnabled: behavior.checkinEnabled,
                engine: EngineKind.local,
                providerBaseUrl: OllamaDefaults.v1URL,
                providerChatModel: "qwen2.5:7b-instruct",
                providerBatchModel: "qwen2.5:7b-instruct",
                providerVisionModel: "qwen2.5vl:7b",
                providerOffline: true
            )
        case .copilot, .none:
            SettingsUpdateRequest(
                captureEnabled: behavior.captureEnabled,
                captureIntervalSeconds: behavior.captureIntervalSeconds,
                checkinEnabled: behavior.checkinEnabled,
                engine: EngineKind.copilotCloud,
                providerOffline: false
            )
        }
        do {
            _ = try await DaemonClient.shared.updateSettings(request)
            try await self.persistPersonalization()
            self.settingsApplied = true
        } catch {
            self.settingsError = error.localizedDescription
            // Block the Launch button on save failure so the user
            // explicitly retries rather than landing in the overlay
            // with their engine choice silently dropped.
            self.settingsApplied = false
        }
    }

    private func persistPersonalization() async throws {
        let trimmedName = self.preferredName.trimmingCharacters(in: .whitespacesAndNewlines)
        if !trimmedName.isEmpty {
            let profiles = try await DaemonClient.shared.listUserProfiles().profiles
            let profileName = trimmedName
            if !profiles.contains(where: { $0.name.caseInsensitiveCompare(profileName) == .orderedSame }) {
                _ = try await DaemonClient.shared.createUserProfile(
                    UserProfileCreateRequest(
                        name: profileName,
                        content: """
                        # About me

                        ## Preferred name

                        \(trimmedName)
                        """,
                        enabled: true
                    )
                )
            }
        }

        let trimmedGoal = self.firstGoal.trimmingCharacters(in: .whitespacesAndNewlines)
        if !trimmedGoal.isEmpty {
            let goals = try await DaemonClient.shared.listGoals().goals
            if !goals.contains(where: { $0.title.caseInsensitiveCompare(trimmedGoal) == .orderedSame }) {
                _ = try await DaemonClient.shared.createGoal(
                    GoalCreateRequest(
                        title: trimmedGoal,
                        summary: L10n.tr("setup.personalize.goal.summary"),
                        whyItMatters: nil,
                        priority: 3
                    )
                )
            }
        }
    }

    private func retrySettings() {
        Task { await self.applyEngineSettings() }
    }
}

extension SettingsUpdateRequest {
    init(
        captureEnabled: Bool? = nil,
        captureIntervalSeconds: Int? = nil,
        checkinEnabled: Bool? = nil,
        engine: String,
        providerBaseUrl: String? = nil,
        providerChatModel: String? = nil,
        providerBatchModel: String? = nil,
        providerVisionModel: String? = nil,
        providerOffline: Bool? = nil
    ) {
        self.init(
            captureEnabled: captureEnabled,
            captureIntervalSeconds: captureIntervalSeconds,
            checkinEnabled: checkinEnabled,
            checkinTimes: nil,
            checkinJitterMinutes: nil,
            conversationInactivityTimeoutSeconds: nil,
            conversationAutoCloseMinutes: nil,
            goalCheckIntervalSeconds: nil,
            mcpEnabled: nil,
            engine: engine,
            providerBaseUrl: PatchField(providerBaseUrl),
            providerChatModel: PatchField(providerChatModel),
            providerBatchModel: PatchField(providerBatchModel),
            providerVisionModel: PatchField(providerVisionModel),
            providerOffline: providerOffline
        )
    }
}
