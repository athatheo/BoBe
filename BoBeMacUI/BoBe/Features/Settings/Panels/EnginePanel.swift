import SwiftUI

/// Engine fields hot-swap via daemon `WorkerRegistry::reload()`; no restart banner.
struct EnginePanel: View {
    @Environment(SettingsStore.self) var store
    @Environment(ExpertMode.self) var expertMode
    @State var availableModels: [ModelInfo] = []
    @State var modelsHint: String?
    /// Non-blocking notice from the daemon — auth needed, plan empty,
    /// transport error, etc. Rendered as an inline banner above the model
    /// picker but never hides the dropdowns. Pair with `modelsAreFallback`
    /// to know whether the list itself is best-effort.
    @State var modelsNotice: ModelsNotice?
    /// True when the daemon returned the static fallback catalog (upstream
    /// was unreachable). The picker is still populated; we just dim each
    /// entry's "unconfigured" badge slightly differently so users know.
    @State var modelsAreFallback = false
    @State var showingSignInSheet = false
    /// Cancelled in `.onDisappear` so the Terminal-sign-in poller doesn't
    /// keep writing into `store.auth` after the user closes Settings. Prior
    /// version leaked an unstructured Task that polled for the full 5 min.
    @State private var authPollTask: Task<Void, Never>?
    @Environment(\.theme) var theme

    /// Read-through to the store so the rest of the file stays terse.
    var settings: DaemonSettings? {
        self.store.settings
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text(L10n.tr("settings.engine.description"))
                    .bobeTextStyle(.settingsBody)
                    .foregroundStyle(self.theme.colors.textMuted)
                    .fixedSize(horizontal: false, vertical: true)

                if let error = self.store.error {
                    SettingsErrorBanner(message: error)
                }

                if let savedMessage = self.store.savedMessage {
                    SettingsSavedToast(message: savedMessage)
                }

                if self.settings != nil {
                    self.engineToggleSection
                    if self.currentEngine == EngineKind.copilotCloud {
                        self.cloudAuthSection
                    } else {
                        self.localProviderSection
                    }
                    self.modelsSection
                    self.offlineSection
                } else if self.store.isLoading {
                    HStack(spacing: 8) {
                        BobeSpinner(size: 14)
                        Text(L10n.tr("settings.engine.loading"))
                            .bobeTextStyle(.settingsBody)
                            .foregroundStyle(self.theme.colors.textMuted)
                    }
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.top, 40)
                }
            }
            .padding(24)
        }
        .task { await self.loadAll() }
        .onDisappear {
            self.authPollTask?.cancel()
            self.authPollTask = nil
            self.store.cancelToast()
        }
        .sheet(isPresented: self.$showingSignInSheet) {
            CopilotSignInSheet(
                onSuccess: {
                    self.showingSignInSheet = false
                    Task { await self.store.loadAuth() }
                },
                onClose: {
                    self.showingSignInSheet = false
                    Task { await self.store.loadAuth() }
                }
            )
        }
    }

    // MARK: - Sections

    var engineToggleSection: some View {
        CollapsibleSection(
            title: L10n.tr("settings.engine.mode.title"),
            icon: "arrow.triangle.2.circlepath",
            description: L10n.tr("settings.engine.mode.description")
        ) {
            VStack(alignment: .leading, spacing: 8) {
                self.modeRow(value: EngineKind.copilotCloud, title: L10n.tr("settings.engine.mode.cloud"))
                self.modeRow(value: EngineKind.local, title: L10n.tr("settings.engine.mode.local"))
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

    /// GitHub sign-in is short and always relevant when on the cloud engine
    /// — collapsing it just hid the "Sign in" button behind a chevron and
    /// pushed the affordance to the visual center of the panel. Flat
    /// header + left-aligned content reads as part of the engine flow
    /// instead of a secondary modal.
    var cloudAuthSection: some View {
        FlatSettingsSection(
            icon: "person.crop.circle.fill",
            title: L10n.tr("settings.engine.auth.title"),
            description: L10n.tr("settings.engine.auth.description")
        ) {
            if let auth = self.store.auth {
                if auth.isAuthenticated {
                    HStack(spacing: 8) {
                        Image(systemName: "checkmark.seal.fill")
                            .foregroundStyle(self.theme.colors.success)
                        Text(L10n.tr("settings.engine.auth.signed_in"))
                            .bobeTextStyle(.settingsBody)
                            .fontWeight(.medium)
                            .foregroundStyle(self.theme.colors.text)
                        if let login = auth.login {
                            Text("@\(login)")
                                .bobeTextStyle(.body)
                                .foregroundStyle(self.theme.colors.textMuted)
                        }
                    }
                } else {
                    VStack(alignment: .leading, spacing: 8) {
                        HStack(spacing: 8) {
                            Image(systemName: "key.horizontal")
                                .foregroundStyle(self.theme.colors.warning)
                            Text(L10n.tr("settings.engine.auth.signed_out"))
                                .bobeTextStyle(.settingsBody)
                                .fontWeight(.medium)
                                .foregroundStyle(self.theme.colors.text)
                        }
                        HStack(spacing: 8) {
                            Button(L10n.tr("settings.engine.auth.sign_in")) {
                                self.showingSignInSheet = true
                            }
                            .buttonStyle(.bordered)
                            if self.expertMode.isEnabled {
                                Button(L10n.tr("settings.engine.auth.sign_in_terminal")) {
                                    self.openSignInTerminal()
                                }
                                .buttonStyle(.borderless)
                                .bobeTextStyle(.helper)
                            }
                        }
                    }
                }
            } else if let authError = self.store.authError {
                Label(authError, systemImage: "exclamationmark.triangle.fill")
                    .bobeTextStyle(.body)
                    .foregroundStyle(self.theme.colors.error)
            } else {
                HStack(spacing: 8) {
                    BobeSpinner(size: 12)
                    Text(L10n.tr("settings.engine.auth.checking"))
                        .bobeTextStyle(.body)
                        .foregroundStyle(self.theme.colors.textMuted)
                }
            }
            Button(L10n.tr("settings.engine.auth.refresh")) {
                Task { await self.store.loadAuth() }
            }
            .buttonStyle(.borderless)
            .bobeTextStyle(.helper)
        }
    }

    /// Local provider config. Flat layout matches cloudAuthSection and
    /// offlineSection so the engine flow doesn't look visually scrambled
    /// between collapsible and non-collapsible siblings.
    var localProviderSection: some View {
        FlatSettingsSection(
            icon: "macbook",
            title: L10n.tr("settings.engine.local.title"),
            description: L10n.tr("settings.engine.local.description")
        ) {
            SettingsRow(
                label: L10n.tr("settings.engine.local.base_url"),
                description: L10n.tr("settings.engine.local.base_url.description")
            ) {
                // Width cap so the TextField doesn't go greedy and squeeze
                // the label column — previously both columns asked for
                // infinity and Grid distributed unpredictably.
                BobeTextField(
                    placeholder: OllamaDefaults.v1URL,
                    text: self.store.optionalBinding(\.providerBaseUrl, fallback: OllamaDefaults.v1URL),
                    width: 260
                )
            }
        }
    }

    var modelsSection: some View {
        CollapsibleSection(
            title: L10n.tr("settings.engine.models.title"),
            icon: "cube.box.fill",
            description: L10n.tr("settings.engine.models.description")
        ) {
            VStack(alignment: .leading, spacing: 12) {
                if let notice = self.modelsNotice {
                    self.modelsNoticeBanner(notice)
                }
                if let hint = self.modelsHint {
                    HStack(spacing: 6) {
                        Image(systemName: "info.circle")
                            .foregroundStyle(self.theme.colors.warning)
                        Text(hint)
                            .bobeTextStyle(.helper)
                            .foregroundStyle(self.theme.colors.textMuted)
                            .fixedSize(horizontal: false, vertical: true)
                        Spacer(minLength: 0)
                    }
                    .padding(10)
                    .background(
                        RoundedRectangle(cornerRadius: 8)
                            .fill(self.theme.colors.tertiary.opacity(0.08))
                    )
                }
                // Always render the dropdowns when we have models — even
                // when the list is a fallback catalog. Users can pick "auto"
                // or a specific entry; the runtime handles entitlement
                // mismatches via `set_model_failed_continuing`.
                if !self.availableModels.isEmpty {
                    self.modelDropdowns
                }
            }
        }
    }

    /// Per-task dropdowns — chat / batch / vision. Hidden behind the
    /// "Show all models" Expert toggle so novice users only see the
    /// Recommended chat model by default.
    @ViewBuilder
    private var modelDropdowns: some View {
        if self.expertMode.isEnabled {
            self.modelDropdown(
                label: L10n.tr("settings.engine.models.chat"),
                description: L10n.tr("settings.engine.models.chat.description"),
                keyPath: \.providerChatModel,
                reasoningKeyPath: \.providerChatReasoning,
                visionOnly: false
            )
            self.modelDropdown(
                label: L10n.tr("settings.engine.models.batch"),
                description: L10n.tr("settings.engine.models.batch.description"),
                keyPath: \.providerBatchModel,
                reasoningKeyPath: \.providerBatchReasoning,
                visionOnly: false
            )
            self.modelDropdown(
                label: L10n.tr("settings.engine.models.vision"),
                description: L10n.tr("settings.engine.models.vision.description"),
                keyPath: \.providerVisionModel,
                reasoningKeyPath: \.providerVisionReasoning,
                visionOnly: true
            )
        } else {
            self.modelDropdown(
                label: L10n.tr("settings.engine.models.recommended"),
                description: L10n.tr("settings.engine.models.recommended.description"),
                keyPath: \.providerChatModel,
                reasoningKeyPath: \.providerChatReasoning,
                visionOnly: false
            )
            HStack(spacing: 6) {
                Image(systemName: "lock.shield")
                    .font(.system(size: 10))
                Text(L10n.tr("settings.engine.models.expert_hint"))
                    .bobeTextStyle(.helper)
                    .fixedSize(horizontal: false, vertical: true)
                Spacer(minLength: 0)
            }
            .foregroundStyle(self.theme.colors.textMuted)
            .padding(.top, 4)
        }
    }

    /// Inline, non-blocking banner above the picker. Mirrors the
    /// `ModelsNotice.code` from the daemon onto title + body + actions, but
    /// never hides the dropdowns — users can always pick a model.
    @ViewBuilder
    private func modelsNoticeBanner(_ notice: ModelsNotice) -> some View {
        let kind = ModelsNoticeKind.from(code: notice.code)
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                Image(systemName: kind.icon)
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(self.theme.colors.primary)
                Text(L10n.tr(kind.titleKey))
                    .bobeTextStyle(.body)
                    .fontWeight(.semibold)
                    .foregroundStyle(self.theme.colors.text)
                Spacer()
            }
            Text(L10n.tr(kind.bodyKey))
                .bobeTextStyle(.helper)
                .foregroundStyle(self.theme.colors.textMuted)
                .fixedSize(horizontal: false, vertical: true)
            HStack(spacing: 8) {
                if kind.showsSignIn {
                    Button(L10n.tr("settings.engine.auth.sign_in")) {
                        self.showingSignInSheet = true
                    }
                    .bobeButton(.primary, size: .small)
                }
                if kind.showsManagePlan {
                    Button(L10n.tr("settings.engine.models.cta.manage_plan")) {
                        if let url = URL(string: "https://github.com/settings/copilot") {
                            NSWorkspace.shared.open(url)
                        }
                    }
                    .bobeButton(.ghost, size: .small)
                }
                if kind.showsSwitchLocal, self.currentEngine != EngineKind.local {
                    Button(L10n.tr("settings.engine.models.cta.switch_local")) {
                        self.applyEngineMode(EngineKind.local)
                    }
                    .bobeButton(.ghost, size: .small)
                }
                Button(L10n.tr("app.common.retry")) {
                    Task { await self.loadModels() }
                }
                .bobeButton(.ghost, size: .small)
                Spacer()
            }
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 8)
                .fill(self.theme.colors.primary.opacity(0.08))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(self.theme.colors.primary.opacity(0.35), lineWidth: 1)
        )
    }

    /// Telemetry/offline section. Flat header to match the rest of the
    /// engine flow (the cloudAuthSection above is also flat). Copy is
    /// engine-aware so we never tell a cloud user "fully offline" — that
    /// would be a lie. Defaults to ON either way: BoBe should not phone
    /// home unless the user opts in.
    var offlineSection: some View {
        let isCloud = self.currentEngine == EngineKind.copilotCloud
        return FlatSettingsSection(
            icon: isCloud ? "shield.lefthalf.filled" : "wifi.slash",
            title: isCloud
                ? L10n.tr("settings.engine.privacy.cloud.title")
                : L10n.tr("settings.engine.privacy.local.title"),
            description: isCloud
                ? L10n.tr("settings.engine.privacy.cloud.description")
                : L10n.tr("settings.engine.privacy.local.description")
        ) {
            SettingsRow(
                label: isCloud
                    ? L10n.tr("settings.engine.privacy.cloud.toggle")
                    : L10n.tr("settings.engine.privacy.local.toggle"),
                description: isCloud
                    ? L10n.tr("settings.engine.privacy.cloud.toggle.description")
                    : L10n.tr("settings.engine.privacy.local.toggle.description")
            ) {
                BobeToggle(
                    isOn: self.store.binding(\.providerOffline, fallback: true),
                    accessibilityLabel: L10n.tr("settings.engine.privacy.toggle")
                )
            }
        }
    }

    // MARK: - Helpers

    var currentEngine: String {
        self.store.settings?.engine ?? EngineKind.copilotCloud
    }

    private func applyEngineMode(_ mode: String) {
        guard let current = self.store.settings, current.engine != mode else { return }
        Task {
            // User-initiated flip — bypass debounce so the registry rebuilds now.
            await self.store.updateImmediate { $0.engine = mode }
            await self.loadModels()
        }
    }

    private func loadAll() async {
        // Settings must land first — `loadModels` reads `currentEngine`
        // which falls back to cloud when settings aren't loaded yet. Once
        // settings are in, auth + models are independent and can run in
        // parallel so the pane opens faster on cold cache.
        await self.store.loadIfNeeded()
        async let auth: Void = self.store.loadAuth()
        async let models: Void = self.loadModels()
        _ = await (auth, models)
    }

    func loadModels() async {
        self.availableModels = []
        self.modelsHint = nil
        self.modelsNotice = nil
        self.modelsAreFallback = false
        // The daemon's `/models` endpoint never errors — it always returns
        // 200 with a list (real or fallback) and an optional notice. So
        // this is a single fetch with a single retry on outright network
        // failure. No 4xx-vs-5xx branching, no error CTAs.
        for attempt in 0 ..< 2 {
            do {
                let resp = try await DaemonClient.shared.listModels(engine: self.currentEngine)
                self.availableModels = resp.models
                self.modelsAreFallback = resp.source == "fallback"
                self.modelsNotice = resp.notice
                if resp.models.isEmpty, resp.notice == nil {
                    self.modelsHint = self.currentEngine == EngineKind.local
                        ? L10n.tr("settings.engine.models.local_empty")
                        : L10n.tr("settings.engine.models.cloud_empty")
                }
                return
            } catch {
                if attempt == 0 {
                    try? await Task.sleep(for: .milliseconds(500))
                    continue
                }
                // Network failure to OUR daemon (not upstream). Surface as a
                // generic hint — at this point the daemon itself isn't
                // reachable, which is a separate failure surface from
                // upstream Copilot.
                self.modelsHint = String(
                    format: L10n.tr("settings.engine.models.error_format"),
                    error.localizedDescription
                )
            }
        }
    }

    private func openSignInTerminal() {
        CopilotSignIn.openLogin(cliPath: self.store.auth?.cliPath)
        // Poll /auth/status after Terminal hand-off so the pane auto-refreshes.
        self.startAuthPolling()
    }

    private func startAuthPolling() {
        self.authPollTask?.cancel()
        self.authPollTask = Task { @MainActor in
            let interval = Duration.seconds(2)
            let deadline = ContinuousClock.now + .seconds(300)
            while !Task.isCancelled, ContinuousClock.now < deadline {
                try? await Task.sleep(for: interval)
                if Task.isCancelled { break }
                await self.store.loadAuth()
                if self.store.auth?.isAuthenticated == true { return }
            }
        }
    }
}

// CopilotSignIn lives in `Services/CopilotSignIn.swift`.

/// Maps a daemon `ModelsNotice.code` onto title / body / icon / action set.
/// Lives next to the panel because the kind enum is purely a rendering
/// concern — the wire-level code stays a plain string.
enum ModelsNoticeKind {
    case authRequired
    case noEntitlements
    case sdkError
    case upstreamUnreachable
    /// Daemon sent a code we don't recognise. Render the generic SDK_ERROR
    /// copy as a safe default rather than dropping the banner entirely.
    case unknown

    static func from(code: String) -> ModelsNoticeKind {
        switch code {
        case "AUTH_REQUIRED": .authRequired
        case "NO_ENTITLEMENTS": .noEntitlements
        case "SDK_ERROR": .sdkError
        case "UPSTREAM_UNREACHABLE": .upstreamUnreachable
        default: .unknown
        }
    }

    var icon: String {
        switch self {
        case .authRequired: "person.badge.key"
        case .noEntitlements: "creditcard"
        case .sdkError, .unknown: "exclamationmark.icloud"
        case .upstreamUnreachable: "wifi.exclamationmark"
        }
    }

    var titleKey: String {
        switch self {
        case .authRequired: "settings.engine.models.cta.auth_required.title"
        case .noEntitlements: "settings.engine.models.cta.no_entitlements.title"
        case .sdkError, .unknown: "settings.engine.models.cta.sdk_error.title"
        case .upstreamUnreachable: "settings.engine.models.cta.unreachable.title"
        }
    }

    var bodyKey: String {
        switch self {
        case .authRequired: "settings.engine.models.cta.auth_required.body"
        case .noEntitlements: "settings.engine.models.cta.no_entitlements.body"
        case .sdkError, .unknown: "settings.engine.models.cta.sdk_error.body"
        case .upstreamUnreachable: "settings.engine.models.cta.unreachable.body"
        }
    }

    /// True when the banner should expose a Sign-in button (auth flows /
    /// generic SDK errors where re-auth is the most common cure).
    var showsSignIn: Bool {
        switch self {
        case .authRequired, .sdkError, .unknown: true
        case .noEntitlements, .upstreamUnreachable: false
        }
    }

    /// True when the banner should expose a "Manage plan" link out to GitHub.
    var showsManagePlan: Bool {
        switch self {
        case .noEntitlements: true
        case .authRequired, .sdkError, .upstreamUnreachable, .unknown: false
        }
    }

    /// True when the banner should expose a "Use local instead" escape hatch.
    /// Surfaces on cloud-side errors so the user always has an exit path.
    var showsSwitchLocal: Bool {
        switch self {
        case .noEntitlements, .sdkError, .unknown: true
        case .authRequired, .upstreamUnreachable: false
        }
    }
}
