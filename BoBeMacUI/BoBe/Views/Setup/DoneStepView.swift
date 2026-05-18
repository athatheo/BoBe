import SwiftUI

struct DoneStepView: View {
    let engineChoice: EngineChoice?
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
                if let error = self.settingsError {
                    VStack(spacing: 6) {
                        Text(error)
                            .bobeTextStyle(.helper)
                            .foregroundStyle(self.theme.colors.primary)
                            .multilineTextAlignment(.center)
                        Button("Retry", action: self.retrySettings)
                            .bobeButton(.secondary, size: .small)
                    }
                }
            }

            Spacer()

            Button(L10n.tr("setup.done.launch"), action: self.onLaunch)
                .bobeButton(.primary, size: .regular)
                .keyboardShortcut(.defaultAction)
                .disabled(!self.settingsApplied)
        }
        .task {
            await self.applyEngineSettings()
        }
    }

    private func applyEngineSettings() async {
        let request = switch self.engineChoice {
        case .local:
            SettingsUpdateRequest(
                engine: EngineKind.local,
                providerBaseUrl: OllamaDefaults.v1URL,
                providerChatModel: "qwen2.5:7b-instruct",
                providerBatchModel: "qwen2.5:7b-instruct",
                providerVisionModel: "qwen2.5vl:7b",
                providerOffline: true
            )
        case .copilot, .none:
            SettingsUpdateRequest(engine: EngineKind.copilotCloud, providerOffline: false)
        }
        do {
            _ = try await DaemonClient.shared.updateSettings(request)
            self.settingsApplied = true
        } catch {
            self.settingsError = error.localizedDescription
            // Block the Launch button on save failure so the user
            // explicitly retries rather than landing in the overlay
            // with their engine choice silently dropped.
            self.settingsApplied = false
        }
    }

    private func retrySettings() {
        Task { await self.applyEngineSettings() }
    }
}

extension SettingsUpdateRequest {
    init(
        engine: String,
        providerBaseUrl: String? = nil,
        providerChatModel: String? = nil,
        providerBatchModel: String? = nil,
        providerVisionModel: String? = nil,
        providerOffline: Bool? = nil
    ) {
        self.init(
            captureEnabled: nil,
            captureIntervalSeconds: nil,
            checkinEnabled: nil,
            checkinTimes: nil,
            checkinJitterMinutes: nil,
            conversationInactivityTimeoutSeconds: nil,
            conversationAutoCloseMinutes: nil,
            goalCheckIntervalSeconds: nil,
            mcpEnabled: nil,
            engine: engine,
            providerBaseUrl: providerBaseUrl,
            providerChatModel: providerChatModel,
            providerBatchModel: providerBatchModel,
            providerVisionModel: providerVisionModel,
            providerOffline: providerOffline
        )
    }
}
