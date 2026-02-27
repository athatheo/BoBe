import AppKit
import OSLog
import SwiftUI

private let logger = Logger(subsystem: "com.bobe.app", category: "SetupWizard")

struct SetupWizard: View {
    @State private var step: SetupStep = .welcome
    @State private var setupMode: SetupMode = .local
    @State private var selectedLocalModel = "small"
    @State private var selectedOpenAIModel = defaultOpenAIModelOption.modelName
    @State private var apiKey = ""
    @State private var onlineProvider = "openai"
    @State private var onlineModel = onlineProviders[0].defaultModel
    @State private var endpoint = ""
    @State private var showLocal = false
    @State private var showOnline = false
    @State private var progressPercent: Double = 0
    @State private var progressMessage = ""
    @State private var errorMessage = ""
    @State private var busy = false
    @State private var captureSkipped = false
    @State private var visionDownloaded = false
    @State private var visionProgress: Double = 0
    @State private var visionMessage = ""
    @State private var visionDownloading = false
    @State private var visionError = ""
    @State private var screenPermission = "not-determined"
    @State private var permissionPollTask: Task<Void, Never>?
    @Environment(\.theme) private var theme

    private enum SetupMode { case local, online }

    private var selectedModelOption: ModelOption {
        localModelOptions.first { $0.id == selectedLocalModel } ?? defaultModelOption
    }

    var body: some View {
        VStack(spacing: 0) {
            // Header
            HStack(spacing: 6) {
                if let logoURL = Bundle.main.url(forResource: "logo-64", withExtension: "png"),
                   let nsImage = NSImage(contentsOf: logoURL) {
                    Image(nsImage: nsImage)
                        .resizable()
                        .frame(width: 28, height: 28)
                }
                Text("BoBe")
                    .font(.system(size: 18, weight: .bold))
                    .foregroundStyle(theme.colors.primary)
            }
            .padding(.top, 24)

            Spacer()

            Group {
                switch step {
                case .welcome: welcomeView
                case .chooseMode: chooseModeView
                case .downloadingEngine, .downloadingModel, .initializing: downloadingView
                case .captureSetup: captureSetupView
                case .complete: completeView
                case .error: errorView
                }
            }
            .padding(.horizontal, 32)

            Spacer()
        }
        .frame(width: 540, height: 680)
        .background(theme.colors.background)
        .onDisappear { permissionPollTask?.cancel() }
        .onChange(of: step) { _, newStep in
            permissionPollTask?.cancel()
            if newStep == .captureSetup {
                checkScreenPermission()
                startPermissionPolling()
                autoStartVisionDownload()
            }
        }
    }

    private var welcomeView: some View {
        VStack(spacing: 0) {
            Text("Welcome to BoBe")
                .font(.system(size: 24, weight: .semibold))
                .foregroundStyle(theme.colors.text)
                .padding(.bottom, 4)

            Text("Your proactive AI companion for Mac.")
                .font(.system(size: 14))
                .foregroundStyle(Color(red: 0.55, green: 0.6, blue: 0.49))
                .padding(.bottom, 20)

            VStack(alignment: .leading, spacing: 14) {
                WelcomeBullet(
                    title: "Reaches out when it can help",
                    desc: "BoBe notices what you're working on and starts conversations when it spots something useful — you don't have to ask."
                )
                WelcomeBullet(
                    title: "Learns your goals and context",
                    desc: "Over time it remembers your projects, habits, and priorities so its help gets more relevant."
                )
                WelcomeBullet(
                    title: "Private by design",
                    desc: "Everything runs on your Mac. Your data stays local and is never uploaded."
                )
            }
            .padding(.bottom, 24)

            Button("Get Started") { step = .chooseMode }
                .buttonStyle(.borderedProminent).tint(theme.colors.primary).controlSize(.large)
        }
        .frame(maxWidth: 420)
    }

    private var chooseModeView: some View {
        ScrollView(.vertical, showsIndicators: false) {
            VStack(alignment: .leading, spacing: 0) {
                Text("Set Up AI")
                    .font(.system(size: 24, weight: .semibold))
                    .foregroundStyle(theme.colors.text)
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.bottom, 4)

                Text("BoBe needs an AI model to think with. The fastest way is connecting your OpenAI key.")
                    .font(.system(size: 13))
                    .foregroundStyle(Color(red: 0.55, green: 0.6, blue: 0.49))
                    .multilineTextAlignment(.center)
                    .frame(maxWidth: 380, alignment: .center)
                    .frame(maxWidth: .infinity)
                    .padding(.bottom, 16)

                // OpenAI section
                Text("OpenAI (recommended):")
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(theme.colors.text)
                    .padding(.bottom, 8)

                Text("OpenAI API Key").font(.system(size: 11, weight: .medium))
                    .foregroundStyle(theme.colors.textMuted).padding(.bottom, 2)
                SecureField("sk-...", text: $apiKey)
                    .textFieldStyle(.roundedBorder).padding(.bottom, 6)

                Text("OpenAI Model").font(.system(size: 11, weight: .medium))
                    .foregroundStyle(theme.colors.textMuted).padding(.bottom, 2)
                Picker("", selection: $selectedOpenAIModel) {
                    ForEach(openAIModelOptions) { m in
                        Text(m.label).tag(m.modelName)
                    }
                }
                .pickerStyle(.menu).labelsHidden().padding(.bottom, 8)

                Button(busy ? "Configuring..." : "Continue with OpenAI") { handleChooseOpenAI() }
                    .buttonStyle(.borderedProminent).tint(theme.colors.primary).controlSize(.large)
                    .disabled(apiKey.trimmingCharacters(in: .whitespaces).isEmpty || busy)
                    .frame(maxWidth: .infinity)
                    .padding(.bottom, 16)

                // Local models collapsible
                SetupCollapsibleSection(title: "Or run AI locally on your Mac", collapsedTitle: "Hide local models", isExpanded: $showLocal) {
                    Text("Download and run an AI model entirely on your Mac. No internet needed after setup.")
                        .font(.system(size: 12)).foregroundStyle(theme.colors.textMuted)

                    ForEach(localModelOptions) { model in
                        ModelRadioCard(model: model, isSelected: selectedLocalModel == model.id) {
                            selectedLocalModel = model.id
                        }
                    }

                    Button(busy ? "Checking connection..." : "Continue with local model") {
                        handleChooseLocal()
                    }
                    .buttonStyle(.borderedProminent).tint(theme.colors.primary).controlSize(.regular)
                    .disabled(busy)
                    .frame(maxWidth: .infinity)
                }
                .padding(.bottom, 12)

                // Other cloud providers collapsible
                SetupCollapsibleSection(
                    title: "Use another cloud provider",
                    collapsedTitle: "Hide more cloud options",
                    isExpanded: $showOnline
                ) {
                    Text("""
                        Connect to another cloud model provider. You can change \
                        this later in BoBe Tuning. Your key is stored in your OS \
                        keychain, not in plain files.
                        """)
                        .font(.system(size: 12)).foregroundStyle(theme.colors.textMuted)

                    Text("Provider").font(.system(size: 11, weight: .medium))
                        .foregroundStyle(theme.colors.textMuted)
                    Picker("", selection: $onlineProvider) {
                        ForEach(onlineProviders) { p in Text(p.label).tag(p.id) }
                    }
                    .pickerStyle(.menu).labelsHidden()
                    .onChange(of: onlineProvider) { _, newId in
                        if let p = onlineProviders.first(where: { $0.id == newId }) {
                            onlineModel = p.defaultModel
                        }
                        endpoint = ""
                    }

                    Text("API Key").font(.system(size: 11, weight: .medium))
                        .foregroundStyle(theme.colors.textMuted)
                    SecureField(
                        onlineProviders.first { $0.id == onlineProvider }?.placeholder ?? "API Key",
                        text: $apiKey
                    ).textFieldStyle(.roundedBorder)

                    if let p = onlineProviders.first(where: { $0.id == onlineProvider }), p.needsEndpoint {
                        Text("Endpoint URL").font(.system(size: 11, weight: .medium))
                            .foregroundStyle(theme.colors.textMuted)
                        TextField(p.endpointPlaceholder, text: $endpoint)
                            .textFieldStyle(.roundedBorder)
                    }

                    Text("Model").font(.system(size: 11, weight: .medium))
                        .foregroundStyle(theme.colors.textMuted)
                    TextField("Model name", text: $onlineModel)
                        .textFieldStyle(.roundedBorder)

                    let provider = onlineProviders.first { $0.id == onlineProvider }
                    let needsEndpoint = provider?.needsEndpoint ?? false
                    let canSubmit = !apiKey.trimmingCharacters(in: .whitespaces).isEmpty
                        && (!needsEndpoint || !endpoint.trimmingCharacters(in: .whitespaces).isEmpty)
                        && !busy

                    Button(busy ? "Configuring..." : "Continue with online LLM") {
                        handleChooseOnline()
                    }
                    .buttonStyle(.borderedProminent).tint(theme.colors.primary).controlSize(.regular)
                    .disabled(!canSubmit)
                    .frame(maxWidth: .infinity)
                }
            }
            .frame(maxWidth: 460)
        }
    }

    private var downloadingView: some View {
        VStack(spacing: 16) {
            VStack(alignment: .leading, spacing: 0) {
                StepIndicator(
                    label: "Downloading AI engine",
                    active: step == .downloadingEngine,
                    done: step == .downloadingModel || step == .initializing
                )
                StepIndicator(
                    label: "Downloading language model",
                    active: step == .downloadingModel,
                    done: step == .initializing
                )
                StepIndicator(
                    label: "Initializing BoBe",
                    active: step == .initializing,
                    done: false
                )
            }

            ProgressView(value: progressPercent, total: 100)
                .progressViewStyle(.linear)
                .frame(width: 320)

            Text("\(Int(progressPercent))%")
                .font(.system(size: 24, weight: .bold, design: .monospaced))
                .foregroundStyle(theme.colors.primary)

            Text(progressMessage)
                .font(.system(size: 13))
                .foregroundStyle(theme.colors.textMuted)
                .multilineTextAlignment(.center)
                .lineLimit(2)
        }
        .frame(maxWidth: 420)
    }

    private var captureSetupView: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Screen Awareness")
                .font(.system(size: 14, weight: .medium)).foregroundStyle(theme.colors.text)
            Text("""
                BoBe glances at your screen periodically so it can offer \
                relevant help, track your goals, and remember what you're \
                working on — without you having to explain everything.
                """)
                .font(.system(size: 12)).foregroundStyle(theme.colors.textMuted)
                .lineSpacing(2)

            // Screen Recording Permission
            PermissionCard(title: "Screen Recording", badge: screenPermission) {
                Text("Grants BoBe access to see what's on your screen.")
                    .font(.system(size: 11)).foregroundStyle(theme.colors.textMuted)
                if screenPermission == "restricted" {
                    Text("This permission is managed by your organization and cannot be changed.")
                        .font(.system(size: 11)).foregroundStyle(.orange)
                }
                if screenPermission != "granted" && screenPermission != "restricted" {
                    Button("Open System Settings") {
                        if let url = URL(
                            string: "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
                        ) {
                            NSWorkspace.shared.open(url)
                        }
                    }
                    .font(.system(size: 11, weight: .medium)).foregroundStyle(theme.colors.primary)
                    .buttonStyle(.plain)
                    Text("Grant permission to continue.")
                        .font(.system(size: 10)).foregroundStyle(theme.colors.textMuted)
                }
            }

            // Vision Model (local only)
            if setupMode == .local {
                PermissionCard(title: "Vision Model", badge: visionBadge) {
                    Text("\(selectedModelOption.visionModel) for screen analysis.")
                        .font(.system(size: 11)).foregroundStyle(theme.colors.textMuted)
                    if visionDownloading {
                        ProgressView(value: visionProgress, total: 100)
                            .progressViewStyle(.linear)
                        Text(visionMessage)
                            .font(.system(size: 10)).foregroundStyle(theme.colors.textMuted)
                    }
                    if !visionError.isEmpty {
                        Text(visionError)
                            .font(.system(size: 10)).foregroundStyle(.red)
                        Button("Retry download") { startVisionDownload() }
                            .font(.system(size: 11, weight: .medium)).foregroundStyle(theme.colors.primary)
                            .buttonStyle(.plain)
                    }
                }
            }

            // Privacy note
            Text(setupMode == .local
                ? "Screenshots are analyzed locally on your Mac and never uploaded."
                : "Screenshots are sent to your AI provider for analysis.")
                .font(.system(size: 11)).foregroundStyle(theme.colors.textMuted).italic()

            // Actions
            HStack(spacing: 10) {
                Button("Skip — disable screen capture") { skipCapture() }
                    .buttonStyle(.bordered).foregroundStyle(theme.colors.textMuted).controlSize(.regular)
                Spacer()
                let visionReady = setupMode != .local || visionDownloaded
                let canContinue = screenPermission == "granted" && visionReady
                Button("Continue") { continueFromCapture() }
                    .buttonStyle(.borderedProminent).tint(theme.colors.primary).controlSize(.regular)
                    .disabled(!canContinue)
            }
        }
        .frame(maxWidth: 440)
    }

    private var visionBadge: String {
        if visionDownloaded { return "granted" }
        if visionDownloading { return "not-determined" }
        if !visionError.isEmpty { return "denied" }
        return "not-determined"
    }

    private var completeView: some View {
        VStack(spacing: 12) {
            Text("You're all set!")
                .font(.system(size: 18, weight: .medium))
                .foregroundStyle(Color(red: 0.55, green: 0.6, blue: 0.49))
                .padding(.bottom, 4)

            VStack(spacing: 6) {
                SummaryRow(
                    label: "AI Model",
                    value: setupMode == .online ? "Cloud LLM" : selectedModelOption.label,
                    ok: true
                )
                SummaryRow(
                    label: "Screen Capture",
                    value: captureSkipped ? "Disabled (skipped)" : "Enabled",
                    ok: !captureSkipped
                )
            }
            .padding(.bottom, 4)

            Text("How BoBe learns about you")
                .font(.system(size: 13, weight: .medium)).foregroundStyle(theme.colors.text)

            InfoCard(
                title: "Your Soul",
                description: """
                    BoBe has a personality guide called a Soul that shapes \
                    how it talks to you — warm, concise, and respectful of \
                    your focus. You can customize it or create new ones.
                    """
            )

            InfoCard(
                title: "Your Goals",
                description: """
                    BoBe learns what you're working toward by listening to \
                    your conversations. It can also detect goals automatically \
                    — like noticing you keep debugging async code and inferring \
                    "Improve async skills." You can add, edit, or remove goals \
                    anytime.
                    """
            )

            // BoBe Tune pointer
            VStack(alignment: .leading, spacing: 4) {
                Text("Find BoBe Tune in your menu bar")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(theme.colors.text)
                Text("""
                    Click the BoBe icon in your menu bar and choose BoBe \
                    Tune to change your AI model, manage Souls and Goals, \
                    toggle screen capture, and adjust all settings.
                    """)
                    .font(.system(size: 12))
                    .foregroundStyle(theme.colors.textMuted).lineSpacing(2)
            }
            .padding(12)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(
                RoundedRectangle(cornerRadius: 10)
                    .fill(theme.colors.surface)
                    .stroke(theme.colors.border, lineWidth: 1)
            )

            Button("Get Started") { completeSetup() }
                .buttonStyle(.borderedProminent).tint(theme.colors.primary).controlSize(.large)
                .padding(.top, 4)
        }
        .frame(maxWidth: 440)
    }

    private var errorView: some View {
        VStack(spacing: 16) {
            Image(systemName: "exclamationmark.triangle.fill")
                .font(.system(size: 40)).foregroundStyle(.red)
            Text("Setup failed").font(.headline).foregroundStyle(theme.colors.text)
            Text(errorMessage)
                .font(.system(size: 13)).foregroundStyle(theme.colors.textMuted)
                .multilineTextAlignment(.center)
            Button("Retry") {
                step = .chooseMode
                errorMessage = ""
                progressPercent = 0
                progressMessage = ""
            }
            .buttonStyle(.borderedProminent).tint(theme.colors.primary)
        }
        .frame(maxWidth: 420)
    }

    private func checkScreenPermission() {
        screenPermission = CGPreflightScreenCaptureAccess() ? "granted" : "not-determined"
    }

    private func startPermissionPolling() {
        permissionPollTask = Task {
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(2))
                guard !Task.isCancelled else { return }
                await MainActor.run { checkScreenPermission() }
            }
        }
    }

    private func handleChooseOpenAI() {
        guard !busy, !apiKey.trimmingCharacters(in: .whitespaces).isEmpty else { return }
        busy = true
        setupMode = .online
        Task {
            defer { busy = false }
            do {
                try await DaemonClient.shared.configureLLM(
                    ConfigureLLMRequest(
                        mode: "openai", model: selectedOpenAIModel,
                        apiKey: apiKey, endpoint: nil
                    )
                )
                apiKey = ""
                step = .captureSetup
            } catch {
                errorMessage = error.localizedDescription
                step = .error
            }
        }
    }

    private func handleChooseOnline() {
        guard !busy, !apiKey.trimmingCharacters(in: .whitespaces).isEmpty else { return }
        let provider = onlineProviders.first { $0.id == onlineProvider }
        let needsEndpoint = provider?.needsEndpoint ?? false
        if needsEndpoint && endpoint.trimmingCharacters(in: .whitespaces).isEmpty { return }

        busy = true
        setupMode = .online
        Task {
            defer { busy = false }
            do {
                try await DaemonClient.shared.configureLLM(
                    ConfigureLLMRequest(
                        mode: onlineProvider, model: onlineModel,
                        apiKey: apiKey,
                        endpoint: needsEndpoint ? endpoint : nil
                    )
                )
                apiKey = ""
                step = .captureSetup
            } catch {
                errorMessage = error.localizedDescription
                step = .error
            }
        }
    }

    private func handleChooseLocal() {
        guard !busy else { return }
        busy = true
        setupMode = .local
        step = .downloadingEngine
        progressMessage = "Downloading AI engine..."
        progressPercent = 0
        Task {
            defer { busy = false }
            do {
                let dataDir = FileManager.default.homeDirectoryForCurrentUser
                    .appendingPathComponent(".bobe")
                try? FileManager.default.createDirectory(at: dataDir, withIntermediateDirectories: true)
                let vals = try dataDir.resourceValues(forKeys: [.volumeAvailableCapacityForImportantUsageKey])
                let available = vals.volumeAvailableCapacityForImportantUsage ?? 0
                if available < selectedModelOption.diskRequirement {
                    let availGB = String(format: "%.1f", Double(available) / 1e9)
                    let reqGB = String(format: "%.1f", Double(selectedModelOption.diskRequirement) / 1e9)
                    throw SetupError.diskSpace("Need ~\(reqGB) GB free, only \(availGB) GB available.")
                }

                try await DaemonClient.shared.configureLLM(
                    ConfigureLLMRequest(
                        mode: "ollama", model: selectedModelOption.modelName,
                        apiKey: nil, endpoint: nil
                    )
                )

                progressMessage = "Setting up Ollama engine..."
                let binaryPath = try await OllamaService.shared.ensureInstalled { percent, message in
                    Task { @MainActor in
                        progressPercent = percent
                        progressMessage = message
                    }
                }
                _ = try await OllamaService.shared.start(binaryPath: binaryPath)
                progressMessage = "Waiting for Ollama to start..."
                let ready = await OllamaService.shared.waitUntilReady()
                if !ready {
                    throw SetupError.general("Ollama failed to start. Please try again.")
                }

                step = .downloadingModel
                progressMessage = "Downloading model..."

                let healthMonitor = Task {
                    while !Task.isCancelled {
                        try? await Task.sleep(for: .seconds(10))
                        guard !Task.isCancelled else { return }
                        let healthy = (try? await DaemonClient.shared.health()) != nil
                        if !healthy {
                            await MainActor.run {
                                errorMessage = "Backend stopped responding during download. Please restart BoBe."
                                step = .error
                            }
                            return
                        }
                    }
                }

                try await DaemonClient.shared.pullModelSSE(
                    model: selectedModelOption.modelName
                ) { status, percent in
                    Task { @MainActor in
                        progressPercent = percent
                        switch status {
                        case "pulling manifest": progressMessage = "Downloading manifest..."
                        case "downloading": progressMessage = "Downloading model... \(Int(percent))%"
                        case "verifying": progressMessage = "Verifying download..."
                        case "success", "complete": progressMessage = "Model ready!"
                        default: progressMessage = status
                        }
                    }
                }

                healthMonitor.cancel()

                step = .initializing
                progressMessage = "Preparing embedding model…"
                progressPercent = 100
                try await DaemonClient.shared.warmupEmbedding()

                var visionSettings = SettingsUpdateRequest()
                visionSettings.visionBackend = "ollama"
                visionSettings.visionOllamaModel = selectedModelOption.visionModel
                _ = try await DaemonClient.shared.updateSettings(visionSettings)

                step = .captureSetup
            } catch {
                errorMessage = error.localizedDescription
                step = .error
            }
        }
    }

    private func autoStartVisionDownload() {
        guard setupMode == .local, !visionDownloaded, !visionDownloading, visionError.isEmpty else { return }
        startVisionDownload()
    }

    private func startVisionDownload() {
        guard !visionDownloading else { return }
        visionDownloading = true
        visionError = ""
        visionProgress = 0
        visionMessage = "Downloading \(selectedModelOption.visionModel)..."
        Task {
            defer { visionDownloading = false }
            do {
                try await DaemonClient.shared.pullModelSSE(
                    model: selectedModelOption.visionModel
                ) { status, percent in
                    Task { @MainActor in
                        visionProgress = percent
                        if status == "downloading" {
                            visionMessage = "Downloading vision model... \(Int(percent))%"
                        } else {
                            visionMessage = status
                        }
                    }
                }
                visionDownloaded = true
                visionMessage = "Vision model ready"
            } catch {
                visionError = error.localizedDescription
                logger.warning("Vision model download failed: \(error.localizedDescription)")
            }
        }
    }

    private func skipCapture() {
        captureSkipped = true
        Task {
            do {
                var settings = SettingsUpdateRequest()
                settings.captureEnabled = false
                settings.visionBackend = "none"
                _ = try await DaemonClient.shared.updateSettings(settings)
                step = .complete
            } catch {
                errorMessage = error.localizedDescription
                step = .error
            }
        }
    }

    private func continueFromCapture() {
        let visionReady = setupMode != .local || visionDownloaded
        Task {
            do {
                if visionReady {
                    var settings = SettingsUpdateRequest()
                    settings.captureEnabled = true
                    if setupMode == .local {
                        settings.visionBackend = "ollama"
                        settings.visionOllamaModel = selectedModelOption.visionModel
                    }
                    _ = try await DaemonClient.shared.updateSettings(settings)
                } else {
                    var settings = SettingsUpdateRequest()
                    settings.captureEnabled = false
                    settings.visionBackend = "none"
                    _ = try await DaemonClient.shared.updateSettings(settings)
                }
            } catch {
                // Non-fatal
            }
            step = .complete
        }
    }

    private func completeSetup() {
        Task {
            do {
                try await DaemonClient.shared.markOnboardingComplete()
                NSApplication.shared.keyWindow?.close()
            } catch {
                errorMessage = error.localizedDescription
                step = .error
            }
        }
    }
}
