import AppKit
import OSLog
import ScreenCaptureKit
import SwiftUI

private let logger = Logger(subsystem: "com.bobe.app", category: "SetupWizard")

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

    // MARK: - Step Views

    private var chooseModeView: some View {
        VStack(spacing: 16) {
            Text("Choose your AI model").font(.headline).foregroundStyle(theme.colors.text)

            ForEach(ModelSize.allCases, id: \.self) { size in
                SetupModelCard(size: size, isSelected: selectedModel == size) { selectedModel = size }
            }

            Button(busy ? "Checking connection..." : "Continue") { startLocalSetup() }
                .buttonStyle(.borderedProminent).tint(theme.colors.primary)
                .controlSize(.large).disabled(busy)

            DisclosureGroup("Or use a cloud LLM") { cloudOptions }
                .font(.subheadline).foregroundStyle(theme.colors.textMuted).padding(.top, 8)
        }
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

    private var downloadingView: some View {
        VStack(spacing: 16) {
            HStack(spacing: 20) {
                SetupStepDot(label: "Engine", done: step != .downloadingEngine)
                SetupStepDot(label: "Model", done: step != .downloadingEngine && step != .downloadingModel)
                SetupStepDot(label: "Features", done: false)
            }

            ProgressView(value: progressPercent, total: 100).progressViewStyle(.linear).frame(width: 300)

            Text(progressMessage).font(.caption).foregroundStyle(theme.colors.textMuted)
                .lineLimit(2).multilineTextAlignment(.center)

            Text("\(Int(progressPercent))%")
                .font(.system(size: 24, weight: .bold, design: .monospaced))
                .foregroundStyle(theme.colors.primary)
        }
    }

    private var captureSetupView: some View {
        VStack(spacing: 14) {
            Text("Screen Capture").font(.headline)
            Text("BoBe watches your screen to understand what you're working on.")
                .font(.subheadline).foregroundStyle(theme.colors.textMuted).multilineTextAlignment(.center)

            SetupFeatureCard(
                title: "Screen Recording Permission",
                description: "Required for screen analysis. Grant in System Settings.",
                granted: screenPermission == "granted",
                badge: screenPermission == "granted" ? "Granted" : "Not Set"
            ) {
                if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture") {
                    NSWorkspace.shared.open(url)
                }
            }

            if let visionModel = selectedModel.separateVisionModel {
                SetupFeatureCard(
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
                SetupFeatureCard(
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

    private var completeView: some View {
        VStack(spacing: 16) {
            Image(systemName: "checkmark.circle.fill").font(.system(size: 48)).foregroundStyle(.green)
            Text("All set! BoBe is ready.").font(.headline)

            VStack(alignment: .leading, spacing: 8) {
                SetupSummaryRow(icon: "checkmark.circle.fill", color: .green,
                           text: "AI Model: \(selectedModel.displayName)")
                SetupSummaryRow(
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

    private var errorView: some View {
        VStack(spacing: 16) {
            Image(systemName: "exclamationmark.triangle.fill").font(.system(size: 40)).foregroundStyle(.red)
            Text("Setup Failed").font(.headline)
            Text(errorMessage).foregroundStyle(theme.colors.textMuted).multilineTextAlignment(.center)
            Button("Retry") { step = .chooseMode; progressPercent = 0; progressMessage = "" }
                .buttonStyle(.borderedProminent).tint(theme.colors.primary)
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
                let dataDir = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".bobe")
                try? FileManager.default.createDirectory(at: dataDir, withIntermediateDirectories: true)
                let vals = try dataDir.resourceValues(forKeys: [.volumeAvailableCapacityForImportantUsageKey])
                let available = vals.volumeAvailableCapacityForImportantUsage ?? 0
                if available < selectedModel.diskRequirement {
                    let availGB = String(format: "%.1f", Double(available) / 1e9)
                    let reqGB = String(format: "%.1f", Double(selectedModel.diskRequirement) / 1e9)
                    throw SetupError.diskSpace("Need ~\(reqGB) GB free, only \(availGB) GB available.")
                }

                try await DaemonClient.shared.configureLLM(
                    ConfigureLLMRequest(mode: "ollama", model: selectedModel.rawValue, apiKey: nil, endpoint: nil)
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
                    throw SetupError.diskSpace("Ollama failed to start. Please try again.")
                }

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

                progressMessage = "Preparing embedding model…"
                progressPercent = 100
                try await DaemonClient.shared.warmupEmbedding()

                var visionSettings = SettingsUpdateRequest()
                visionSettings.visionBackend = "ollama"
                visionSettings.visionOllamaModel = selectedModel.visionModelName
                _ = try await DaemonClient.shared.updateSettings(visionSettings)

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
                step = .captureSetup
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
