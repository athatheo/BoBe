import SwiftUI
import ScreenCaptureKit
import OSLog

private let logger = Logger(subsystem: "com.bobe.app", category: "SetupWizard")

// MARK: - Step Flow

/// Wizard steps — feature-based with skip support
enum SetupStep {
    case chooseMode
    case downloadingEngine
    case downloadingModel
    case captureSetup
    case complete
    case error
}

// MARK: - Model Tiers

/// Model tiers: Small/Medium use unified VL model, Large uses separate LLM + vision
enum ModelSize: String, CaseIterable {
    case small = "qwen3-vl:2b"
    case medium = "qwen3-vl:4b"
    case large = "qwen3:14b"

    var displayName: String {
        switch self {
        case .small: "Small (2B)"
        case .medium: "Medium (4B)"
        case .large: "Large (14B)"
        }
    }

    var sizeDescription: String {
        switch self {
        case .small: "~2.7 GB total"
        case .medium: "~4.1 GB total"
        case .large: "~15 GB total"
        }
    }

    var description: String {
        switch self {
        case .small: "Fast, works on any Mac. Good for getting started."
        case .medium: "Smarter responses. Recommended for 32GB+ RAM."
        case .large: "Best quality. Recommended for 64GB+ RAM."
        }
    }

    /// For Small/Medium the LLM IS the vision model (unified VL)
    var isUnifiedVL: Bool { self != .large }

    /// Separate vision model only needed for Large tier
    var separateVisionModel: String? {
        self == .large ? "qwen3-vl:8b" : nil
    }

    /// Vision model name (same as LLM for small/medium, separate for large)
    var visionModelName: String {
        separateVisionModel ?? rawValue
    }

    var diskRequirement: Int64 {
        switch self {
        case .small: 2_700_000_000
        case .medium: 4_100_000_000
        case .large: 15_000_000_000
        }
    }
}

/// Cloud provider options
enum CloudProvider: String, CaseIterable {
    case openai = "OpenAI"
    case azure = "Azure OpenAI"

    var defaultModel: String {
        switch self {
        case .openai: "gpt-4o-mini"
        case .azure: "gpt-5-mini"
        }
    }
}

// MARK: - Setup Wizard

struct SetupWizard: View {
    @State private var step: SetupStep = .chooseMode
    @State private var selectedModel: ModelSize = .small
    @State private var useCloud = false
    @State private var cloudProvider: CloudProvider = .openai
    @State private var apiKey = ""
    @State private var cloudEndpoint = ""
    @State private var cloudModel = ""
    @State private var progressPercent: Double = 0
    @State private var progressMessage = ""
    @State private var errorMessage = ""
    @State private var busy = false
    // Feature setup tracking
    @State private var captureSkipped = false
    @State private var visionDownloaded = false
    @State private var screenPermission = "not-determined"
    @State private var permissionPollTask: Task<Void, Never>?
    @Environment(\.theme) private var theme

    var body: some View {
        VStack(spacing: 0) {
            VStack(spacing: 8) {
                Text("🧠").font(.system(size: 40))
                Text("BoBe Setup").font(.title2.bold()).foregroundStyle(theme.colors.text)
            }
            .padding(.top, 24)

            Spacer()

            Group {
                switch step {
                case .chooseMode: chooseModeView
                case .downloadingEngine, .downloadingModel: downloadingView
                case .captureSetup: captureSetupView
                case .complete: completeView
                case .error: errorView
                }
            }
            .padding(.horizontal, 32)

            Spacer()
        }
        .frame(width: 540, height: 620)
        .background(theme.colors.background)
        .onChange(of: step) { _, newStep in
            permissionPollTask?.cancel()
            if newStep == .captureSetup {
                checkScreenPermission()
                startPermissionPolling { checkScreenPermission() }
            }
        }
    }

    // MARK: - Permission Checks

    private func checkScreenPermission() {
        screenPermission = CGPreflightScreenCaptureAccess() ? "granted" : "not-determined"
    }

    private func startPermissionPolling(check: @escaping @MainActor () -> Void) {
        permissionPollTask = Task {
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(2))
                await MainActor.run { check() }
            }
        }
    }

    // MARK: - Choose Mode

    private var chooseModeView: some View {
        VStack(spacing: 16) {
            Text("Choose your AI model").font(.headline).foregroundStyle(theme.colors.text)

            ForEach(ModelSize.allCases, id: \.self) { size in
                modelCard(size)
            }

            Button(busy ? "Checking connection..." : "Continue") { startLocalSetup() }
                .buttonStyle(.borderedProminent).tint(theme.colors.primary)
                .controlSize(.large).disabled(busy)

            DisclosureGroup("Or use a cloud LLM") { cloudOptions }
                .font(.subheadline).foregroundStyle(theme.colors.textMuted).padding(.top, 8)
        }
    }

    private func modelCard(_ size: ModelSize) -> some View {
        HStack {
            VStack(alignment: .leading, spacing: 4) {
                Text(size.displayName).font(.system(size: 14, weight: .semibold)).foregroundStyle(theme.colors.text)
                Text(size.sizeDescription).font(.system(size: 11)).foregroundStyle(theme.colors.textMuted)
                Text(size.description).font(.system(size: 11)).foregroundStyle(theme.colors.textMuted)
            }
            Spacer()
            Image(systemName: selectedModel == size ? "checkmark.circle.fill" : "circle")
                .foregroundStyle(selectedModel == size ? theme.colors.primary : theme.colors.border)
        }
        .padding(12)
        .background(
            RoundedRectangle(cornerRadius: 10)
                .fill(theme.colors.surface)
                .stroke(selectedModel == size ? theme.colors.primary : theme.colors.border, lineWidth: 1)
        )
        .onTapGesture { selectedModel = size }
    }

    private var cloudOptions: some View {
        VStack(alignment: .leading, spacing: 10) {
            Picker("Provider", selection: $cloudProvider) {
                ForEach(CloudProvider.allCases, id: \.self) { Text($0.rawValue).tag($0) }
            }
            .pickerStyle(.menu)
            .onChange(of: cloudProvider) { _, p in cloudModel = p.defaultModel }

            SecureField("API Key", text: $apiKey).textFieldStyle(.roundedBorder)

            if cloudProvider == .azure {
                TextField("Endpoint URL", text: $cloudEndpoint).textFieldStyle(.roundedBorder)
            }
            TextField("Model", text: $cloudModel).textFieldStyle(.roundedBorder)

            Button(busy ? "Configuring..." : "Continue with cloud LLM") { startCloudSetup() }
                .buttonStyle(.bordered).disabled(apiKey.isEmpty || busy)
        }
        .padding(.top, 4)
        .onAppear { cloudModel = cloudProvider.defaultModel }
    }

    // MARK: - Downloading

    private var downloadingView: some View {
        VStack(spacing: 16) {
            HStack(spacing: 20) {
                stepDot(label: "Engine", done: step != .downloadingEngine)
                stepDot(label: "Model", done: step != .downloadingEngine && step != .downloadingModel)
                stepDot(label: "Features", done: false)
            }

            ProgressView(value: progressPercent, total: 100).progressViewStyle(.linear).frame(width: 300)

            Text(progressMessage).font(.caption).foregroundStyle(theme.colors.textMuted)
                .lineLimit(2).multilineTextAlignment(.center)

            Text("\(Int(progressPercent))%")
                .font(.system(size: 24, weight: .bold, design: .monospaced))
                .foregroundStyle(theme.colors.primary)
        }
    }

    private func stepDot(label: String, done: Bool) -> some View {
        VStack(spacing: 4) {
            Circle().fill(done ? theme.colors.primary : theme.colors.border).frame(width: 12, height: 12)
                .overlay { if done { Image(systemName: "checkmark").font(.system(size: 7, weight: .bold)).foregroundStyle(.white) } }
            Text(label).font(.system(size: 9)).foregroundStyle(theme.colors.textMuted)
        }
    }

    // MARK: - Step 4: Capture Setup

    private var captureSetupView: some View {
        VStack(spacing: 14) {
            Text("Screen Capture").font(.headline)
            Text("BoBe watches your screen to understand what you're working on.")
                .font(.subheadline).foregroundStyle(theme.colors.textMuted).multilineTextAlignment(.center)

            // Permission card
            featureCard(
                title: "Screen Recording Permission",
                description: "Required for screen analysis. Grant in System Settings.",
                granted: screenPermission == "granted",
                badge: screenPermission == "granted" ? "Granted" : "Not Set"
            ) {
                if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture") {
                    NSWorkspace.shared.open(url)
                }
            }

            // Vision model card
            if let visionModel = selectedModel.separateVisionModel {
                featureCard(
                    title: "Vision Model (\(visionModel))",
                    description: "Analyzes screen content. ~6 GB download.",
                    granted: visionDownloaded,
                    badge: visionDownloaded ? "Downloaded" : (busy ? "Downloading..." : "Not Downloaded")
                ) {
                    downloadVisionModel()
                }
                if busy {
                    ProgressView(value: progressPercent, total: 100).progressViewStyle(.linear)
                    Text(progressMessage).font(.caption).foregroundStyle(theme.colors.textMuted)
                }
            } else {
                featureCard(
                    title: "Vision Model",
                    description: "Already included — your LLM handles vision too.",
                    granted: true, badge: "Included"
                )
            }

            Text("Screen capture can be enabled later in Settings.")
                .font(.caption).foregroundStyle(theme.colors.textMuted)

            HStack(spacing: 12) {
                Button("Skip") { skipCapture() }
                    .buttonStyle(.bordered).foregroundStyle(theme.colors.textMuted)
                Button("Continue") { step = .complete }
                    .buttonStyle(.borderedProminent).tint(theme.colors.primary)
                    .disabled(
                        screenPermission != "granted"
                            || (selectedModel.separateVisionModel != nil && !visionDownloaded && !busy)
                    )
            }
        }
    }

    // MARK: - Step 5: Complete

    private var completeView: some View {
        VStack(spacing: 16) {
            Image(systemName: "checkmark.circle.fill").font(.system(size: 48)).foregroundStyle(.green)
            Text("All set! BoBe is ready.").font(.headline)

            VStack(alignment: .leading, spacing: 8) {
                summaryRow(icon: "checkmark.circle.fill", color: .green,
                           text: "AI Model: \(selectedModel.displayName)")
                summaryRow(
                    icon: captureSkipped ? "exclamationmark.triangle.fill" : "checkmark.circle.fill",
                    color: captureSkipped ? .orange : .green,
                    text: captureSkipped ? "Screen Capture: Disabled (skipped)" : "Screen Capture: Enabled"
                )
            }
            .padding(16)
            .background(RoundedRectangle(cornerRadius: 10).fill(theme.colors.surface))

            if useCloud {
                Text("Your data stays local. Messages are sent to the cloud API.")
                    .font(.caption).foregroundStyle(theme.colors.textMuted)
            } else {
                Text("Everything runs locally on your Mac.")
                    .font(.caption).foregroundStyle(theme.colors.textMuted)
            }

            Button("Get Started") { completeSetup() }
                .buttonStyle(.borderedProminent).tint(theme.colors.primary).controlSize(.large)
        }
    }

    private func summaryRow(icon: String, color: Color, text: String) -> some View {
        HStack(spacing: 8) {
            Image(systemName: icon).foregroundStyle(color).font(.system(size: 14))
            Text(text).font(.system(size: 13)).foregroundStyle(theme.colors.text)
        }
    }

    // MARK: - Error

    private var errorView: some View {
        VStack(spacing: 16) {
            Image(systemName: "exclamationmark.triangle.fill").font(.system(size: 40)).foregroundStyle(.red)
            Text("Setup Failed").font(.headline)
            Text(errorMessage).foregroundStyle(theme.colors.textMuted).multilineTextAlignment(.center)
            Button("Retry") { step = .chooseMode; progressPercent = 0; progressMessage = "" }
                .buttonStyle(.borderedProminent).tint(theme.colors.primary)
        }
    }

    // MARK: - Shared Components

    private func featureCard(
        title: String, description: String, granted: Bool, badge: String,
        action: (() -> Void)? = nil
    ) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text(title).font(.system(size: 13, weight: .semibold))
                Spacer()
                Text(badge)
                    .font(.system(size: 11, weight: .medium))
                    .foregroundStyle(granted ? theme.colors.secondary : theme.colors.textMuted)
                    .padding(.horizontal, 8).padding(.vertical, 2)
                    .background(RoundedRectangle(cornerRadius: 8)
                        .fill(granted ? theme.colors.secondary.opacity(0.15) : theme.colors.surface))
            }
            Text(description).font(.system(size: 11)).foregroundStyle(theme.colors.textMuted)
            if let action, !granted {
                Button("Set Up") { action() }
                    .font(.system(size: 11, weight: .medium)).foregroundStyle(theme.colors.primary).buttonStyle(.plain)
            }
        }
        .padding(12)
        .background(RoundedRectangle(cornerRadius: 10).fill(theme.colors.surface))
        .overlay(RoundedRectangle(cornerRadius: 10).stroke(theme.colors.border, lineWidth: 1))
    }

    // MARK: - Actions

    private func startLocalSetup() {
        guard !busy else { return }
        busy = true
        step = .downloadingEngine
        progressMessage = "Configuring LLM..."
        progressPercent = 0
        Task {
            defer { busy = false }
            do {
                // Disk space check
                let dataDir = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".bobe")
                try? FileManager.default.createDirectory(at: dataDir, withIntermediateDirectories: true)
                let vals = try dataDir.resourceValues(forKeys: [.volumeAvailableCapacityForImportantUsageKey])
                let available = vals.volumeAvailableCapacityForImportantUsage ?? 0
                if available < selectedModel.diskRequirement {
                    let availGB = String(format: "%.1f", Double(available) / 1e9)
                    let reqGB = String(format: "%.1f", Double(selectedModel.diskRequirement) / 1e9)
                    throw SetupError.diskSpace("Need ~\(reqGB) GB free, only \(availGB) GB available.")
                }

                // Configure LLM
                try await DaemonClient.shared.configureLLM(
                    ConfigureLLMRequest(mode: "ollama", model: selectedModel.rawValue, apiKey: nil, endpoint: nil)
                )

                // Ensure Ollama is installed and running
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
                    throw SetupError.diskSpace("Ollama failed to start. Please try again.")
                }

                // Pull LLM
                step = .downloadingModel
                progressMessage = "Downloading model..."
                try await DaemonClient.shared.pullModelSSE(model: selectedModel.rawValue) { status, percent in
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

                // Warmup embedding
                progressMessage = "Downloading embedding model..."
                try await DaemonClient.shared.warmupEmbedding()

                // Configure vision model name (for small/medium it's the same model)
                var visionSettings = SettingsUpdateRequest()
                visionSettings.visionBackend = "ollama"
                visionSettings.visionOllamaModel = selectedModel.visionModelName
                _ = try await DaemonClient.shared.updateSettings(visionSettings)

                // For unified VL models, vision is already downloaded
                if selectedModel.isUnifiedVL { visionDownloaded = true }

                step = .captureSetup
            } catch {
                errorMessage = error.localizedDescription
                step = .error
            }
        }
    }

    private func startCloudSetup() {
        guard !busy else { return }
        busy = true
        useCloud = true
        step = .downloadingEngine
        progressMessage = "Configuring cloud LLM..."
        progressPercent = 0
        Task {
            defer { busy = false }
            do {
                try await DaemonClient.shared.configureLLM(
                    ConfigureLLMRequest(
                        mode: cloudProvider == .azure ? "azure_openai" : cloudProvider.rawValue.lowercased(),
                        model: cloudModel,
                        apiKey: apiKey,
                        endpoint: cloudProvider == .azure ? cloudEndpoint : nil
                    )
                )
                apiKey = ""
                progressMessage = "Downloading embedding model..."
                try await DaemonClient.shared.warmupEmbedding()
                // Cloud mode doesn't have local vision setup.
                captureSkipped = true
                step = .complete
            } catch {
                errorMessage = error.localizedDescription
                step = .error
            }
        }
    }

    private func downloadVisionModel() {
        guard !busy, let visionModel = selectedModel.separateVisionModel else { return }
        busy = true
        progressPercent = 0
        progressMessage = "Downloading vision model..."
        Task {
            defer { busy = false }
            do {
                try await DaemonClient.shared.pullModelSSE(model: visionModel) { status, percent in
                    Task { @MainActor in
                        progressPercent = percent
                        if status == "downloading" { progressMessage = "Downloading vision model... \(Int(percent))%" }
                    }
                }
                visionDownloaded = true
            } catch {
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

private enum SetupError: Error, LocalizedError {
    case diskSpace(String)
    var errorDescription: String? {
        switch self { case .diskSpace(let msg): msg }
    }
}
