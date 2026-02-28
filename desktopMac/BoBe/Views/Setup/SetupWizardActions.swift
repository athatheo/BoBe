import AppKit
import OSLog
import SwiftUI

private let logger = Logger(subsystem: "com.bobe.app", category: "SetupWizard")

// MARK: - Setup Wizard Actions

extension SetupWizard {

    func checkScreenPermission() {
        screenPermission = CGPreflightScreenCaptureAccess() ? "granted" : "not-determined"
    }

    func startPermissionPolling() {
        permissionPollTask = Task {
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(2))
                guard !Task.isCancelled else { return }
                await MainActor.run { checkScreenPermission() }
            }
        }
    }

    func handleChooseOpenAI() {
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

    func handleChooseOnline() {
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

    func handleChooseLocal() {
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
                progressMessage = "Preparing embedding model\u{2026}"
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

    func autoStartVisionDownload() {
        guard setupMode == .local, !visionDownloaded, !visionDownloading, visionError.isEmpty else { return }
        startVisionDownload()
    }

    func startVisionDownload() {
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

    func continueFromCapture() {
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

    func completeSetup() {
        guard !isFinishingSetup else { return }
        isFinishingSetup = true
        Task {
            do {
                try await DaemonClient.shared.markOnboardingComplete()
                let status = try await DaemonClient.shared.getOnboardingStatus()
                guard !status.needsOnboarding else {
                    throw SetupError.general(
                        "Setup could not be saved yet. Please verify your model/API settings and try again."
                    )
                }
                let delegateType = String(describing: type(of: NSApp.delegate))
                logger.info("setup.complete_status_ok delegate_type=\(delegateType, privacy: .public)")
                await MainActor.run {
                    isFinishingSetup = false
                    logger.info("setup.complete_posting_notification")
                    NotificationCenter.default.post(name: .bobeSetupCompleted, object: nil)
                }
            } catch {
                await MainActor.run {
                    isFinishingSetup = false
                    errorMessage = error.localizedDescription
                    logger.error("setup.complete_failed error=\(error.localizedDescription, privacy: .public)")
                    step = .error
                }
            }
        }
    }
}
