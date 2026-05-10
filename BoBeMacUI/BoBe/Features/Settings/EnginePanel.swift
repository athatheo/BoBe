import SwiftUI

/// Settings → Engine. Radio between cloud (GitHub Copilot) and local
/// (Ollama). For each mode, exposes:
/// - Cloud: signed-in @login banner + per-class model dropdowns from
///   `/models?engine=copilot_cloud`.
/// - Local: provider URL + per-class model dropdowns from
///   `/models?engine=local` + offline toggle.
///
/// Engine fields are hot-swap on the daemon side: PATCHing any of them
/// triggers `WorkerRegistry::reload()`. No restart-required banner —
/// we just show "Saved" toast and let the worker registry catch up
/// silently in the background.
struct EnginePanel: View {
    @State private var settings: DaemonSettings?
    @State private var auth: AuthStatusResponse?
    @State private var availableModels: [ModelInfo] = []
    @State private var modelsHint: String?
    @State private var isLoading = false
    @State private var error: String?
    @State private var savedMessage: String?
    @State private var saveTask: Task<Void, Never>?
    @State private var savedToastTask: Task<Void, Never>?
    @Environment(\.theme) private var theme

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text(L10n.tr("settings.engine.title"))
                    .font(.title2.bold())
                    .foregroundStyle(self.theme.colors.text)

                Text(L10n.tr("settings.engine.description"))
                    .font(.system(size: 13))
                    .foregroundStyle(self.theme.colors.textMuted)

                if let error {
                    self.errorBanner(error)
                }

                if let savedMessage {
                    self.savedToast(savedMessage)
                }

                if self.settings != nil {
                    self.engineToggleSection
                    if self.currentEngine == "copilot_cloud" {
                        self.cloudAuthSection
                    } else {
                        self.localProviderSection
                    }
                    self.modelsSection
                    self.offlineSection
                } else if self.isLoading {
                    HStack(spacing: 8) {
                        BobeSpinner(size: 14)
                        Text(L10n.tr("settings.engine.loading"))
                            .font(.system(size: 13))
                            .foregroundStyle(self.theme.colors.textMuted)
                    }
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.top, 40)
                }
            }
            .padding(24)
        }
        .task { await self.loadAll() }
    }

    // MARK: - Sections

    private var engineToggleSection: some View {
        CollapsibleSection(
            title: L10n.tr("settings.engine.mode.title"),
            icon: "arrow.triangle.2.circlepath",
            description: L10n.tr("settings.engine.mode.description")
        ) {
            VStack(alignment: .leading, spacing: 8) {
                self.modeRow(value: "copilot_cloud", title: L10n.tr("settings.engine.mode.cloud"))
                self.modeRow(value: "local", title: L10n.tr("settings.engine.mode.local"))
            }
        }
    }

    private func modeRow(value: String, title: String) -> some View {
        Button {
            self.applyEngineMode(value)
        } label: {
            HStack(spacing: 10) {
                Image(systemName: self.currentEngine == value ? "largecircle.fill.circle" : "circle")
                    .foregroundStyle(self.currentEngine == value
                        ? self.theme.colors.primary
                        : self.theme.colors.border)
                Text(title)
                    .foregroundStyle(self.theme.colors.text)
                Spacer()
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    private var cloudAuthSection: some View {
        CollapsibleSection(
            title: L10n.tr("settings.engine.auth.title"),
            icon: "person.crop.circle.fill",
            description: L10n.tr("settings.engine.auth.description")
        ) {
            VStack(alignment: .leading, spacing: 10) {
                if let auth = self.auth {
                    if auth.isAuthenticated {
                        HStack(spacing: 8) {
                            Image(systemName: "checkmark.seal.fill")
                                .foregroundStyle(self.theme.colors.secondary)
                            Text(L10n.tr("settings.engine.auth.signed_in"))
                                .font(.system(size: 13, weight: .medium))
                                .foregroundStyle(self.theme.colors.text)
                            if let login = auth.login {
                                Text("@\(login)")
                                    .font(.system(size: 12))
                                    .foregroundStyle(self.theme.colors.textMuted)
                            }
                        }
                    } else {
                        VStack(alignment: .leading, spacing: 8) {
                            HStack(spacing: 8) {
                                Image(systemName: "key.horizontal")
                                    .foregroundStyle(self.theme.colors.tertiary)
                                Text(L10n.tr("settings.engine.auth.signed_out"))
                                    .font(.system(size: 13, weight: .medium))
                                    .foregroundStyle(self.theme.colors.text)
                            }
                            Button(L10n.tr("settings.engine.auth.sign_in")) {
                                self.openSignInTerminal()
                            }
                            .buttonStyle(.bordered)
                        }
                    }
                } else {
                    HStack(spacing: 8) {
                        BobeSpinner(size: 12)
                        Text(L10n.tr("settings.engine.auth.checking"))
                            .font(.system(size: 12))
                            .foregroundStyle(self.theme.colors.textMuted)
                    }
                }
                Button(L10n.tr("settings.engine.auth.refresh")) {
                    Task { await self.loadAuth() }
                }
                .buttonStyle(.borderless)
                .font(.system(size: 11))
            }
        }
    }

    private var localProviderSection: some View {
        CollapsibleSection(
            title: L10n.tr("settings.engine.local.title"),
            icon: "macbook",
            description: L10n.tr("settings.engine.local.description")
        ) {
            SettingsRow(
                label: L10n.tr("settings.engine.local.base_url"),
                description: L10n.tr("settings.engine.local.base_url.description")
            ) {
                BobeTextField(
                    placeholder: "http://127.0.0.1:11434/v1",
                    text: self.optionalBinding(\.providerBaseUrl, fallback: "http://127.0.0.1:11434/v1")
                )
            }
        }
    }

    private var modelsSection: some View {
        CollapsibleSection(
            title: L10n.tr("settings.engine.models.title"),
            icon: "cube.box.fill",
            description: L10n.tr("settings.engine.models.description")
        ) {
            VStack(alignment: .leading, spacing: 12) {
                if let hint = self.modelsHint {
                    HStack(spacing: 6) {
                        Image(systemName: "info.circle")
                            .foregroundStyle(self.theme.colors.tertiary)
                        Text(hint)
                            .font(.system(size: 11))
                            .foregroundStyle(self.theme.colors.textMuted)
                        Spacer()
                    }
                    .padding(10)
                    .background(
                        RoundedRectangle(cornerRadius: 8)
                            .fill(self.theme.colors.tertiary.opacity(0.08))
                    )
                }
                self.modelDropdown(
                    label: L10n.tr("settings.engine.models.chat"),
                    description: L10n.tr("settings.engine.models.chat.description"),
                    keyPath: \.providerChatModel,
                    visionOnly: false
                )
                self.modelDropdown(
                    label: L10n.tr("settings.engine.models.batch"),
                    description: L10n.tr("settings.engine.models.batch.description"),
                    keyPath: \.providerBatchModel,
                    visionOnly: false
                )
                self.modelDropdown(
                    label: L10n.tr("settings.engine.models.vision"),
                    description: L10n.tr("settings.engine.models.vision.description"),
                    keyPath: \.providerVisionModel,
                    visionOnly: true
                )
            }
        }
    }

    private func modelDropdown(
        label: String,
        description: String,
        keyPath: WritableKeyPath<DaemonSettings, String?>,
        visionOnly: Bool
    ) -> some View {
        let pool = visionOnly ? self.availableModels.filter(\.vision) : self.availableModels
        // The "—" sentinel means "use the CLI's default model."
        let options: [String] = ["—"] + pool.map(\.id)
        let displayName = { (id: String) -> String in
            if id == "—" {
                return L10n.tr("settings.engine.models.use_default")
            }
            return self.availableModels.first(where: { $0.id == id })?.name ?? id
        }

        let binding = Binding<String>(
            get: {
                let raw = self.settings?[keyPath: keyPath]
                // Server may return `nil` (never set) or `""` (cleared
                // via the empty-string sentinel) — both render as `—`.
                if let raw, !raw.isEmpty { return raw }
                return "—"
            },
            set: { newValue in
                guard var current = self.settings else { return }
                // Swift's `JSONEncoder` omits `nil` Optional fields by
                // default — sending `nil` here would silently no-op on
                // the daemon's PATCH handler. Use empty string as the
                // clear sentinel; the daemon's `set_opt_string!` macro
                // normalizes `""` back to `None` in Config.
                current[keyPath: keyPath] = (newValue == "—") ? "" : newValue
                self.settings = current
                self.debounceSave()
            }
        )

        return SettingsRow(label: label, description: description) {
            BobeMenuPicker(
                selection: binding,
                options: options,
                label: displayName,
                width: 280
            )
        }
    }

    private var offlineSection: some View {
        CollapsibleSection(
            title: L10n.tr("settings.engine.offline.title"),
            icon: "wifi.slash",
            description: L10n.tr("settings.engine.offline.description")
        ) {
            SettingsRow(
                label: L10n.tr("settings.engine.offline.toggle"),
                description: L10n.tr("settings.engine.offline.toggle.description")
            ) {
                BobeToggle(isOn: self.binding(\.providerOffline, fallback: true))
            }
        }
    }

    // MARK: - Helpers

    private var currentEngine: String {
        self.settings?.engine ?? "copilot_cloud"
    }

    private func errorBanner(_ message: String) -> some View {
        HStack(spacing: 6) {
            Image(systemName: "exclamationmark.triangle.fill")
                .foregroundStyle(self.theme.colors.primary)
            Text(message)
                .font(.system(size: 12))
                .foregroundStyle(self.theme.colors.primary)
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 8).fill(self.theme.colors.primary.opacity(0.08)))
    }

    private func savedToast(_ message: String) -> some View {
        HStack(spacing: 6) {
            Image(systemName: "checkmark.circle.fill")
                .foregroundStyle(self.theme.colors.secondary)
            Text(message)
                .font(.system(size: 11))
                .foregroundStyle(self.theme.colors.secondary)
            Spacer()
        }
        .transition(.opacity)
    }

    private func binding<V>(
        _ keyPath: WritableKeyPath<DaemonSettings, V>,
        fallback: @autoclosure @escaping () -> V
    ) -> Binding<V> {
        Binding(
            get: { self.settings?[keyPath: keyPath] ?? fallback() },
            set: { newValue in
                guard var current = self.settings else { return }
                current[keyPath: keyPath] = newValue
                self.settings = current
                self.debounceSave()
            }
        )
    }

    private func optionalBinding(
        _ keyPath: WritableKeyPath<DaemonSettings, String?>,
        fallback: @autoclosure @escaping () -> String
    ) -> Binding<String> {
        Binding(
            get: { self.settings?[keyPath: keyPath] ?? fallback() },
            set: { newValue in
                guard var current = self.settings else { return }
                current[keyPath: keyPath] = newValue.isEmpty ? nil : newValue
                self.settings = current
                self.debounceSave()
            }
        )
    }

    private func applyEngineMode(_ mode: String) {
        guard var current = self.settings else { return }
        guard current.engine != mode else { return }
        current.engine = mode
        self.settings = current
        Task {
            // Engine mode flip is a deliberate user action; save without
            // debounce so the registry rebuilds quickly.
            self.saveTask?.cancel()
            await self.persist()
        }
        // Reload model list for the new engine.
        Task { await self.loadModels() }
    }

    private func debounceSave() {
        self.saveTask?.cancel()
        self.saveTask = Task {
            try? await Task.sleep(for: .seconds(0.6))
            guard !Task.isCancelled else { return }
            await self.persist()
        }
    }

    private func persist() async {
        guard let settings = self.settings else { return }
        var req = SettingsUpdateRequest()
        req.engine = settings.engine
        req.providerBaseUrl = settings.providerBaseUrl
        req.providerChatModel = settings.providerChatModel
        req.providerBatchModel = settings.providerBatchModel
        req.providerVisionModel = settings.providerVisionModel
        req.providerOffline = settings.providerOffline
        do {
            let resp = try await DaemonClient.shared.updateSettings(req)
            if resp.persistFailed == true {
                self.error = L10n.tr("settings.shared.action.persist_failed")
                self.savedMessage = nil
            } else {
                self.error = nil
                self.savedMessage = resp.message
                self.scheduleSavedToastDismiss()
            }
        } catch {
            self.error = error.localizedDescription
            self.savedMessage = nil
        }
    }

    private func scheduleSavedToastDismiss() {
        self.savedToastTask?.cancel()
        self.savedToastTask = Task {
            try? await Task.sleep(for: .seconds(2.4))
            guard !Task.isCancelled else { return }
            self.savedMessage = nil
        }
    }

    private func loadAll() async {
        self.isLoading = true
        defer { isLoading = false }
        do {
            self.settings = try await DaemonClient.shared.getSettings()
            await self.loadAuth()
            await self.loadModels()
        } catch {
            self.error = error.localizedDescription
        }
    }

    private func loadAuth() async {
        self.auth = nil
        do {
            self.auth = try await DaemonClient.shared.getAuthStatus()
        } catch {
            // Don't surface — auth probe is best-effort. If the daemon
            // can't reach the CLI, the settings pane still works for
            // configuring everything else.
        }
    }

    private func loadModels() async {
        self.availableModels = []
        self.modelsHint = nil
        do {
            let resp = try await DaemonClient.shared.listModels(engine: self.currentEngine)
            self.availableModels = resp.models
            if resp.models.isEmpty {
                self.modelsHint = self.currentEngine == "local"
                    ? L10n.tr("settings.engine.models.local_empty")
                    : L10n.tr("settings.engine.models.cloud_empty")
            }
        } catch let DaemonError.httpError(statusCode, _) where statusCode == 503 {
            // Expected: Ollama not running yet (local mode without
            // setup) or Copilot CLI not reachable.
            self.modelsHint = self.currentEngine == "local"
                ? L10n.tr("settings.engine.models.local_unavailable")
                : L10n.tr("settings.engine.models.cloud_unavailable")
        } catch {
            // Daemon down, parse error, network issue — surface it so
            // the user knows the dropdowns aren't empty by design.
            self.modelsHint = String(
                format: L10n.tr("settings.engine.models.error_format"),
                error.localizedDescription
            )
        }
    }

    private func openSignInTerminal() {
        CopilotSignIn.openLogin(cliPath: self.auth?.cliPath)
        // Same pattern as the wizard: once we hand off to Terminal,
        // poll /auth/status in the background so the pane auto-refreshes
        // when the user finishes signing in.
        self.startAuthPolling()
    }

    private func startAuthPolling() {
        Task { @MainActor in
            let interval = Duration.seconds(2)
            let deadline = ContinuousClock.now + .seconds(300)
            while !Task.isCancelled, ContinuousClock.now < deadline {
                try? await Task.sleep(for: interval)
                if Task.isCancelled { break }
                do {
                    let resp = try await DaemonClient.shared.getAuthStatus()
                    self.auth = resp
                    if resp.isAuthenticated { return }
                } catch {
                    // Transient — keep polling.
                }
            }
        }
    }
}

/// Helpers for kicking off the Copilot CLI sign-in flow from Swift.
/// Shared between the welcome wizard's `CloudAuthStepView` and the
/// Settings → Engine pane.
///
/// The Copilot CLI is bundled inside BoBe (via the SDK's `embedded-cli`
/// feature) and extracted to `~/.cache/github-copilot-sdk-{ver}/copilot`
/// on first `Client::start()`. Its sign-in is interactive (GitHub device
/// flow: prints a code, asks the user to enter it at github.com/login/
/// device, polls for the token, writes it to the macOS keychain).
///
/// We can't drive that programmatically from Swift, so the workflow is:
/// 1. Daemon's `/auth/status` returns the extracted CLI's absolute path.
/// 2. We open Terminal.app and run that binary directly (no args — the
///    CLI's interactive prompt handles the device flow on first run).
/// 3. User completes the flow in Terminal; auth is persisted to keychain.
/// 4. User clicks "Check again" in the wizard / Settings; `/auth/status`
///    now reports `is_authenticated: true`.
///
/// `gh` is intentionally NOT used. Copilot CLI and `gh` are separate
/// tools — Copilot CLI has its own auth flow and its own keychain entry.
/// We bundle the binary the user needs; no external install required.
enum CopilotSignIn {
    /// Open Terminal.app and run the bundled Copilot CLI's interactive
    /// prompt. The CLI handles auth on first run via GitHub's device
    /// flow. If `cliPath` is `nil` (daemon hasn't started a Client yet
    /// — unusual), we just open Terminal with no command so the user
    /// can run whatever Copilot CLI they have on PATH themselves.
    static func openLogin(cliPath: String?) {
        // Shell-escape the path: wrap in single quotes and escape any
        // embedded single quotes. macOS user paths don't normally
        // contain quotes, but defensive coding is cheap here.
        let command: String
        if let cliPath {
            let escaped = cliPath.replacingOccurrences(of: "'", with: "'\\''")
            command = "'\(escaped)'"
        } else {
            command = "copilot"
        }
        let script = """
        tell application "Terminal"
            activate
            do script "\(command)"
        end tell
        """
        let process = Process()
        process.launchPath = "/usr/bin/osascript"
        process.arguments = ["-e", script]
        try? process.run()
    }
}
