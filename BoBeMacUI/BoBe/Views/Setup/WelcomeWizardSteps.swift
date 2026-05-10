import AppKit
import CoreGraphics
import SwiftUI

// MARK: - Welcome step

struct WelcomeStepView: View {
    let onContinue: () -> Void
    @Environment(\.theme) private var theme

    var body: some View {
        VStack(spacing: 24) {
            Spacer()

            ZStack {
                Circle()
                    .fill(self.theme.colors.primary.opacity(0.15))
                    .frame(width: 88, height: 88)
                Image(systemName: "sparkles")
                    .font(.system(size: 38, weight: .light))
                    .foregroundStyle(self.theme.colors.primary)
            }

            VStack(spacing: 12) {
                Text(L10n.tr("setup.welcome.title"))
                    .bobeTextStyle(.setupTitle)
                    .foregroundStyle(self.theme.colors.text)

                Text(L10n.tr("setup.welcome.body"))
                    .bobeTextStyle(.setupBody)
                    .foregroundStyle(self.theme.colors.textMuted)
                    .multilineTextAlignment(.center)
                    .lineSpacing(4)
            }

            Spacer()

            Button(L10n.tr("setup.welcome.continue"), action: self.onContinue)
                .bobeButton(.primary, size: .regular)
                .keyboardShortcut(.defaultAction)
        }
    }
}

// MARK: - Engine choice step
//
// Asks the user how BoBe should think: GitHub Copilot subscription
// (cloud) or a local AI model running on their Mac. Selection is held
// in the parent wizard's `@State`; the daemon learns about it on the
// Done step via PATCH /settings.

struct EngineChoiceStepView: View {
    @Binding var selection: EngineChoice?
    let onContinue: () -> Void

    @Environment(\.theme) private var theme

    var body: some View {
        VStack(spacing: 18) {
            VStack(spacing: 8) {
                Text(L10n.tr("setup.engine.title"))
                    .bobeTextStyle(.setupTitle)
                    .foregroundStyle(self.theme.colors.text)
                Text(L10n.tr("setup.engine.body"))
                    .bobeTextStyle(.setupBody)
                    .foregroundStyle(self.theme.colors.textMuted)
                    .multilineTextAlignment(.center)
            }
            .padding(.top, 8)

            VStack(spacing: 12) {
                EngineChoiceCard(
                    icon: "cloud.fill",
                    title: L10n.tr("setup.engine.copilot.title"),
                    subtitle: L10n.tr("setup.engine.copilot.subtitle"),
                    isSelected: self.selection == .copilot
                ) {
                    self.selection = .copilot
                }

                EngineChoiceCard(
                    icon: "macbook",
                    title: L10n.tr("setup.engine.local.title"),
                    subtitle: L10n.tr("setup.engine.local.subtitle"),
                    isSelected: self.selection == .local
                ) {
                    self.selection = .local
                }
            }

            Text(L10n.tr("setup.engine.footer"))
                .bobeTextStyle(.helper)
                .foregroundStyle(self.theme.colors.textMuted)
                .multilineTextAlignment(.center)

            Spacer()

            Button(L10n.tr("setup.welcome.continue"), action: self.onContinue)
                .bobeButton(.primary, size: .regular)
                .keyboardShortcut(.defaultAction)
                .disabled(self.selection == nil)
        }
    }
}

private struct EngineChoiceCard: View {
    let icon: String
    let title: String
    let subtitle: String
    let isSelected: Bool
    let onTap: () -> Void

    @Environment(\.theme) private var theme
    @State private var isHovered = false

    var body: some View {
        Button(action: self.onTap) {
            HStack(alignment: .top, spacing: 14) {
                Image(systemName: self.icon)
                    .font(.system(size: 24, weight: .light))
                    .foregroundStyle(self.isSelected ? self.theme.colors.primary : self.theme.colors.textMuted)
                    .frame(width: 36, height: 36)

                VStack(alignment: .leading, spacing: 4) {
                    Text(self.title)
                        .bobeTextStyle(.setupHeading)
                        .foregroundStyle(self.theme.colors.text)
                    Text(self.subtitle)
                        .font(.system(size: 12))
                        .foregroundStyle(self.theme.colors.textMuted)
                        .lineLimit(2)
                        .multilineTextAlignment(.leading)
                }
                .frame(maxWidth: .infinity, alignment: .leading)

                Image(systemName: self.isSelected ? "largecircle.fill.circle" : "circle")
                    .font(.system(size: 18))
                    .foregroundStyle(self.isSelected ? self.theme.colors.primary : self.theme.colors.border)
                    .padding(.top, 6)
            }
            .padding(14)
            .background(
                RoundedRectangle(cornerRadius: 12)
                    .fill(self.isSelected
                        ? self.theme.colors.primary.opacity(0.08)
                        : self.theme.colors.surface)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 12)
                    .stroke(
                        self.isSelected
                            ? self.theme.colors.primary
                            : (self.isHovered ? self.theme.colors.primary.opacity(0.5) : self.theme.colors.border),
                        lineWidth: self.isSelected ? 1.5 : 1
                    )
            )
        }
        .buttonStyle(.plain)
        .onHover { self.isHovered = $0 }
    }
}

// MARK: - Permissions step

private enum PermissionState {
    case unknown
    case granted
    case denied
}

struct PermissionsStepView: View {
    let onContinue: () -> Void

    @State private var state: PermissionState = .unknown
    @Environment(\.theme) private var theme

    var body: some View {
        VStack(spacing: 20) {
            VStack(spacing: 8) {
                Text(L10n.tr("setup.permissions.title"))
                    .bobeTextStyle(.setupTitle)
                    .foregroundStyle(self.theme.colors.text)
                Text(L10n.tr("setup.permissions.body"))
                    .bobeTextStyle(.setupBody)
                    .foregroundStyle(self.theme.colors.textMuted)
                    .multilineTextAlignment(.center)
            }
            .padding(.top, 12)

            Spacer()

            self.statusCard

            Spacer()

            self.actionRow
        }
        .onAppear { self.refreshState() }
    }

    @ViewBuilder
    private var statusCard: some View {
        switch self.state {
        case .unknown:
            EmptyView()
        case .granted:
            VStack(spacing: 10) {
                Image(systemName: "checkmark.seal.fill")
                    .font(.system(size: 32))
                    .foregroundStyle(self.theme.colors.secondary)
                Text(L10n.tr("setup.permissions.granted"))
                    .bobeTextStyle(.setupHeading)
                    .foregroundStyle(self.theme.colors.text)
            }
            .frame(maxWidth: .infinity)
            .padding(20)
            .background(self.cardBackground(color: self.theme.colors.secondary.opacity(0.5)))
        case .denied:
            VStack(spacing: 10) {
                Image(systemName: "lock.fill")
                    .font(.system(size: 32))
                    .foregroundStyle(self.theme.colors.tertiary)
                Text(L10n.tr("setup.permissions.denied"))
                    .bobeTextStyle(.setupHeading)
                    .foregroundStyle(self.theme.colors.text)
                Text(L10n.tr("setup.permissions.denied_hint"))
                    .bobeTextStyle(.helper)
                    .foregroundStyle(self.theme.colors.textMuted)
                    .multilineTextAlignment(.center)
            }
            .frame(maxWidth: .infinity)
            .padding(20)
            .background(self.cardBackground(color: self.theme.colors.tertiary.opacity(0.5)))
        }
    }

    @ViewBuilder
    private var actionRow: some View {
        switch self.state {
        case .unknown:
            HStack(spacing: 10) {
                Button(L10n.tr("setup.permissions.grant"), action: self.requestAccess)
                    .bobeButton(.primary, size: .regular)
                    .keyboardShortcut(.defaultAction)
                Button(L10n.tr("setup.permissions.skip"), action: self.onContinue)
                    .bobeButton(.ghost, size: .small)
            }
        case .granted:
            Button(L10n.tr("setup.welcome.continue"), action: self.onContinue)
                .bobeButton(.primary, size: .regular)
                .keyboardShortcut(.defaultAction)
        case .denied:
            VStack(spacing: 8) {
                Button(L10n.tr("setup.permissions.open_settings")) {
                    if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture") {
                        NSWorkspace.shared.open(url)
                    }
                }
                .bobeButton(.primary, size: .regular)
                .keyboardShortcut(.defaultAction)
                Button(L10n.tr("setup.permissions.skip"), action: self.onContinue)
                    .bobeButton(.ghost, size: .small)
            }
        }
    }

    private func cardBackground(color: Color) -> some View {
        RoundedRectangle(cornerRadius: 12)
            .fill(color.opacity(0.12))
            .overlay(
                RoundedRectangle(cornerRadius: 12)
                    .stroke(color, lineWidth: 1)
            )
    }

    private func refreshState() {
        if CGPreflightScreenCaptureAccess() {
            self.state = .granted
        } else {
            self.state = .unknown
        }
    }

    private func requestAccess() {
        let granted = CGRequestScreenCaptureAccess()
        if granted {
            self.state = .granted
        } else {
            self.state = .denied
        }
    }
}

// MARK: - Cloud auth step
//
// Shown when the user picked the cloud (GitHub Copilot) engine. Hits
// the daemon's `/auth/status` endpoint which wraps the SDK's
// `Client::get_auth_status()`. If the user has authed Copilot CLI
// before (via `gh auth login` or `copilot login`), they see "signed in
// as @login" and continue. Otherwise we offer a path to sign in.

private enum CloudAuthState {
    case checking
    case authenticated(login: String?, authType: String?)
    case unauthenticated(message: String?)
    case error(message: String)
}

struct CloudAuthStepView: View {
    let onContinue: () -> Void

    @State private var state: CloudAuthState = .checking
    @Environment(\.theme) private var theme

    var body: some View {
        VStack(spacing: 20) {
            VStack(spacing: 8) {
                Text(L10n.tr("setup.cloud_auth.title"))
                    .bobeTextStyle(.setupTitle)
                    .foregroundStyle(self.theme.colors.text)
                Text(L10n.tr("setup.cloud_auth.body"))
                    .bobeTextStyle(.setupBody)
                    .foregroundStyle(self.theme.colors.textMuted)
                    .multilineTextAlignment(.center)
            }
            .padding(.top, 12)

            Spacer()

            self.statusCard

            Spacer()

            self.actionRow
        }
        .task { await self.refresh() }
    }

    @ViewBuilder
    private var statusCard: some View {
        switch self.state {
        case .checking:
            VStack(spacing: 10) {
                ProgressView()
                    .scaleEffect(0.9)
                Text(L10n.tr("setup.cloud_auth.checking"))
                    .bobeTextStyle(.helper)
                    .foregroundStyle(self.theme.colors.textMuted)
            }
            .frame(maxWidth: .infinity)
            .padding(20)

        case let .authenticated(login, _):
            VStack(spacing: 10) {
                Image(systemName: "checkmark.seal.fill")
                    .font(.system(size: 32))
                    .foregroundStyle(self.theme.colors.secondary)
                Text(L10n.tr("setup.cloud_auth.signed_in"))
                    .bobeTextStyle(.setupHeading)
                    .foregroundStyle(self.theme.colors.text)
                if let login {
                    Text("@\(login)")
                        .bobeTextStyle(.helper)
                        .foregroundStyle(self.theme.colors.textMuted)
                }
            }
            .frame(maxWidth: .infinity)
            .padding(20)
            .background(self.cardBackground(color: self.theme.colors.secondary.opacity(0.5)))

        case let .unauthenticated(message):
            VStack(spacing: 10) {
                Image(systemName: "key.horizontal")
                    .font(.system(size: 32))
                    .foregroundStyle(self.theme.colors.tertiary)
                Text(L10n.tr("setup.cloud_auth.signed_out"))
                    .bobeTextStyle(.setupHeading)
                    .foregroundStyle(self.theme.colors.text)
                Text(message ?? L10n.tr("setup.cloud_auth.signed_out_hint"))
                    .bobeTextStyle(.helper)
                    .foregroundStyle(self.theme.colors.textMuted)
                    .multilineTextAlignment(.center)
            }
            .frame(maxWidth: .infinity)
            .padding(20)
            .background(self.cardBackground(color: self.theme.colors.tertiary.opacity(0.5)))

        case let .error(message):
            VStack(spacing: 10) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .font(.system(size: 32))
                    .foregroundStyle(self.theme.colors.primary)
                Text(L10n.tr("setup.cloud_auth.error"))
                    .bobeTextStyle(.setupHeading)
                    .foregroundStyle(self.theme.colors.text)
                Text(message)
                    .bobeTextStyle(.helper)
                    .foregroundStyle(self.theme.colors.textMuted)
                    .multilineTextAlignment(.center)
            }
            .frame(maxWidth: .infinity)
            .padding(20)
            .background(self.cardBackground(color: self.theme.colors.primary.opacity(0.5)))
        }
    }

    @ViewBuilder
    private var actionRow: some View {
        switch self.state {
        case .checking:
            EmptyView()
        case .authenticated:
            Button(L10n.tr("setup.welcome.continue"), action: self.onContinue)
                .bobeButton(.primary, size: .regular)
                .keyboardShortcut(.defaultAction)
        case .unauthenticated, .error:
            VStack(spacing: 8) {
                Button(L10n.tr("setup.cloud_auth.open_terminal"), action: self.openSignInTerminal)
                    .bobeButton(.primary, size: .regular)
                Button(L10n.tr("setup.cloud_auth.retry")) {
                    Task { await self.refresh() }
                }
                .bobeButton(.secondary, size: .small)
                Button(L10n.tr("setup.cloud_auth.skip"), action: self.onContinue)
                    .bobeButton(.ghost, size: .small)
            }
        }
    }

    private func cardBackground(color: Color) -> some View {
        RoundedRectangle(cornerRadius: 12)
            .fill(color.opacity(0.12))
            .overlay(
                RoundedRectangle(cornerRadius: 12)
                    .stroke(color, lineWidth: 1)
            )
    }

    private func refresh() async {
        await MainActor.run { self.state = .checking }
        do {
            let resp = try await DaemonClient.shared.getAuthStatus()
            await MainActor.run {
                if resp.isAuthenticated {
                    self.state = .authenticated(login: resp.login, authType: resp.authType)
                } else {
                    self.state = .unauthenticated(message: resp.statusMessage)
                }
            }
        } catch {
            await MainActor.run {
                self.state = .error(message: error.localizedDescription)
            }
        }
    }

    /// Opens Terminal.app and runs `gh auth login --scopes copilot` so
    /// the user can authenticate. The bundled CLI shares its auth state
    /// with the system `gh` install — once they're logged in there, the
    /// next "Retry check" lands on `.authenticated`.
    private func openSignInTerminal() {
        let script = """
        tell application "Terminal"
            activate
            do script "gh auth login --scopes copilot"
        end tell
        """
        let process = Process()
        process.launchPath = "/usr/bin/osascript"
        process.arguments = ["-e", script]
        try? process.run()
    }
}

// MARK: - Local setup step
//
// Shown when the user picked the local engine. Triggers
// `POST /local-runtime/install` with default model selections, then
// streams the `/local-runtime/status` SSE channel into three progress
// bars (Ollama runtime / chat model / vision model). Cancellable.

private struct LocalDefaults {
    static let chatModel = "qwen2.5:7b-instruct"
    static let batchModel = "qwen2.5:7b-instruct"
    static let visionModel = "qwen2.5vl:7b"
}

struct LocalSetupStepView: View {
    let onContinue: () -> Void

    @State private var snapshot: LocalRuntimeSnapshot?
    @State private var ramGB: Int? = systemMemoryGB()
    @State private var streamTask: Task<Void, Never>?
    @State private var hasStarted = false
    @State private var startError: String?
    @Environment(\.theme) private var theme

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            VStack(alignment: .leading, spacing: 8) {
                Text(L10n.tr("setup.local.title"))
                    .bobeTextStyle(.setupTitle)
                    .foregroundStyle(self.theme.colors.text)
                Text(L10n.tr("setup.local.body"))
                    .bobeTextStyle(.setupBody)
                    .foregroundStyle(self.theme.colors.textMuted)
            }

            self.hardwareCard

            VStack(spacing: 12) {
                self.progressRow(
                    title: L10n.tr("setup.local.runtime"),
                    progress: self.snapshot?.runtime,
                    status: nil
                )
                self.modelRow(
                    title: L10n.tr("setup.local.chat_model"),
                    pull: self.snapshot?.chatModel
                )
                self.modelRow(
                    title: L10n.tr("setup.local.vision_model"),
                    pull: self.snapshot?.visionModel
                )
            }

            if let error = self.startError {
                Text(error)
                    .bobeTextStyle(.helper)
                    .foregroundStyle(self.theme.colors.primary)
            }

            Spacer()

            self.actionRow
        }
        .task {
            await self.startInstall()
        }
        .onDisappear {
            self.streamTask?.cancel()
        }
    }

    @ViewBuilder
    private var hardwareCard: some View {
        if let ramGB = self.ramGB, ramGB < 16 {
            HStack(spacing: 10) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .foregroundStyle(self.theme.colors.tertiary)
                Text(L10n.tr("setup.local.ram_warning"))
                    .bobeTextStyle(.helper)
                    .foregroundStyle(self.theme.colors.textMuted)
            }
            .padding(10)
            .background(
                RoundedRectangle(cornerRadius: 8)
                    .fill(self.theme.colors.tertiary.opacity(0.1))
            )
        }
    }

    @ViewBuilder
    private func progressRow(
        title: String,
        progress: LocalRuntimeDownload?,
        status: String?
    ) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(title)
                    .bobeTextStyle(.setupHeading)
                Spacer()
                Text(progress.map { "\($0.percent ?? 0)%" } ?? "—")
                    .bobeTextStyle(.helper)
                    .foregroundStyle(self.theme.colors.textMuted)
            }
            BobeLinearProgressBar(progress: Double(progress?.percent ?? 0) / 100.0)
            if let status {
                Text(status)
                    .bobeTextStyle(.helper)
                    .foregroundStyle(self.theme.colors.textMuted)
            }
        }
    }

    @ViewBuilder
    private func modelRow(
        title: String,
        pull: LocalRuntimePull?
    ) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(title)
                    .bobeTextStyle(.setupHeading)
                Spacer()
                Text(pull.map { "\($0.percent ?? 0)%" } ?? "—")
                    .bobeTextStyle(.helper)
                    .foregroundStyle(self.theme.colors.textMuted)
            }
            BobeLinearProgressBar(progress: Double(pull?.percent ?? 0) / 100.0)
            if let status = pull?.status, !status.isEmpty {
                Text(status)
                    .bobeTextStyle(.helper)
                    .foregroundStyle(self.theme.colors.textMuted)
            }
        }
    }

    @ViewBuilder
    private var actionRow: some View {
        let status = self.snapshot?.status ?? "running"
        switch status {
        case "complete":
            Button(L10n.tr("setup.welcome.continue"), action: self.onContinue)
                .bobeButton(.primary, size: .regular)
                .keyboardShortcut(.defaultAction)
        case "canceled":
            Button(L10n.tr("setup.local.retry")) {
                Task { await self.startInstall() }
            }
            .bobeButton(.primary, size: .regular)
        case "failed":
            VStack(spacing: 8) {
                if let error = self.snapshot?.error {
                    Text(error)
                        .bobeTextStyle(.helper)
                        .foregroundStyle(self.theme.colors.primary)
                        .multilineTextAlignment(.center)
                }
                Button(L10n.tr("setup.local.retry")) {
                    Task { await self.startInstall() }
                }
                .bobeButton(.primary, size: .regular)
            }
        default:
            Button(L10n.tr("setup.local.cancel"), action: self.cancelInstall)
                .bobeButton(.secondary, size: .small)
        }
    }

    private func startInstall() async {
        self.startError = nil
        let request = LocalRuntimeInstallRequest(
            chatModel: LocalDefaults.chatModel,
            batchModel: LocalDefaults.batchModel,
            visionModel: LocalDefaults.visionModel
        )
        do {
            _ = try await DaemonClient.shared.startLocalRuntimeInstall(request)
            self.hasStarted = true
        } catch {
            // 409 just means an install is already in flight — that's
            // fine, we still want to subscribe to status.
            if !error.localizedDescription.lowercased().contains("conflict") {
                self.startError = error.localizedDescription
            }
        }
        self.streamTask?.cancel()
        self.streamTask = Task { @MainActor in
            do {
                try await DaemonClient.shared.streamLocalRuntimeStatus { snap in
                    Task { @MainActor in
                        self.snapshot = snap
                    }
                }
            } catch {
                self.startError = error.localizedDescription
            }
        }
    }

    private func cancelInstall() {
        Task {
            try? await DaemonClient.shared.cancelLocalRuntimeInstall()
        }
    }
}

private func systemMemoryGB() -> Int? {
    var memory: UInt64 = 0
    var size = MemoryLayout<UInt64>.size
    let result = sysctlbyname("hw.memsize", &memory, &size, nil, 0)
    guard result == 0 else { return nil }
    return Int(memory / (1024 * 1024 * 1024))
}

// MARK: - Done step

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
                    Text(error)
                        .bobeTextStyle(.helper)
                        .foregroundStyle(self.theme.colors.primary)
                        .multilineTextAlignment(.center)
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

    /// PATCH /settings with the engine choice (and per-class model
    /// defaults for the local branch). Daemon's hot-swap listener
    /// rebuilds the worker registry against the new config without an
    /// app restart.
    private func applyEngineSettings() async {
        let request: SettingsUpdateRequest
        switch self.engineChoice {
        case .local:
            request = SettingsUpdateRequest(
                engine: "local",
                providerBaseUrl: "http://127.0.0.1:11434/v1",
                providerChatModel: "qwen2.5:7b-instruct",
                providerBatchModel: "qwen2.5:7b-instruct",
                providerVisionModel: "qwen2.5vl:7b",
                providerOffline: true
            )
        case .copilot, .none:
            request = SettingsUpdateRequest(engine: "copilot_cloud", providerOffline: false)
        }
        do {
            _ = try await DaemonClient.shared.updateSettings(request)
            self.settingsApplied = true
        } catch {
            self.settingsError = error.localizedDescription
            // Don't block — user can still launch the app and fix
            // settings from Settings → Engine.
            self.settingsApplied = true
        }
    }
}

// MARK: - SettingsUpdateRequest convenience init for the wizard

private extension SettingsUpdateRequest {
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
