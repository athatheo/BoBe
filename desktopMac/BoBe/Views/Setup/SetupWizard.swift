import OSLog
import ScreenCaptureKit
import SwiftUI

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
    @State var step: SetupStep = .chooseMode
    @State var selectedModel: ModelSize = .small
    @State var useCloud = false
    @State var cloudProvider: CloudProvider = .openai
    @State var apiKey = ""
    @State var cloudEndpoint = ""
    @State var cloudModel = ""
    @State var progressPercent: Double = 0
    @State var progressMessage = ""
    @State var errorMessage = ""
    @State var busy = false
    // Feature setup tracking
    @State var captureSkipped = false
    @State var visionDownloaded = false
    @State var screenPermission = "not-determined"
    @State private var permissionPollTask: Task<Void, Never>?
    @Environment(\.theme) var theme

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

    // MARK: - Actions

    func startLocalSetup() {
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
                progressMessage = "Preparing embedding model…"
                progressPercent = 100 // Show full bar during warmup
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

    func startCloudSetup() {
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
                // Cloud users should still set up screen capture
                step = .captureSetup
            } catch {
                errorMessage = error.localizedDescription
                step = .error
            }
        }
    }

    func downloadVisionModel() {
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

    func skipCapture() {
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

    func completeSetup() {
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

enum SetupError: Error, LocalizedError {
    case diskSpace(String)
    var errorDescription: String? {
        switch self { case .diskSpace(let msg): msg }
    }
}
