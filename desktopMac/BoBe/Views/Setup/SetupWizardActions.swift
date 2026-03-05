import AppKit
import OSLog
import SwiftUI

private let logger = Logger(subsystem: "com.bobe.app", category: "SetupWizard")

extension SetupWizard {
    func checkScreenPermission() {
        if CGPreflightScreenCaptureAccess() {
            screenPermission = .granted
        } else {
            if !hasRequestedCaptureAccess {
                CGRequestScreenCaptureAccess()
                hasRequestedCaptureAccess = true
            }
            screenPermission = .notDetermined
        }
    }

    func startPermissionPolling() {
        permissionPollTask = Task {
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(2))
                guard !Task.isCancelled else { return }
                await MainActor.run { self.checkScreenPermission() }
            }
        }
    }

    func handleCloudSetup() {
        let trimmedApiKey = apiKey.trimmingCharacters(in: .whitespacesAndNewlines)
        let trimmedEndpoint = endpoint.trimmingCharacters(in: .whitespacesAndNewlines)
        let trimmedDeployment = deployment.trimmingCharacters(in: .whitespacesAndNewlines)
        let trimmedModel = selectedModel.trimmingCharacters(in: .whitespacesAndNewlines)

        guard !busy, !trimmedApiKey.isEmpty else { return }
        let provider = options?.cloudProviders.first { $0.id == selectedProvider }
        if provider?.needsEndpoint == true, trimmedEndpoint.isEmpty { return }
        if provider?.needsDeployment == true, trimmedDeployment.isEmpty { return }

        busy = true
        setupMode = .online
        var request = SetupRequest(mode: "cloud")
        request.provider = selectedProvider
        request.apiKey = trimmedApiKey
        request.model = trimmedModel.isEmpty ? nil : trimmedModel
        if provider?.needsEndpoint == true { request.endpoint = trimmedEndpoint }
        if provider?.needsDeployment == true { request.deployment = trimmedDeployment }
        self.startSetupJob(request)
    }

    func handleLocalSetup() {
        guard !busy else { return }
        busy = true
        setupMode = .local
        var request = SetupRequest(mode: "local")
        request.tier = selectedTier
        self.startSetupJob(request)
    }

    func startSetupJob(_ request: SetupRequest) {
        step = .setupInProgress
        progressMessage = L10n.tr("setup.progress.starting")
        progressPercent = 0
        Task {
            do {
                let job = try await DaemonClient.shared.startSetupJob(request)
                await MainActor.run { setupJob = job }
                self.startPolling(jobId: job.jobId)
            } catch {
                await MainActor.run {
                    busy = false
                    errorMessage = error.localizedDescription
                    step = .error
                }
            }
        }
    }

    func startPolling(jobId: String) {
        pollTask?.cancel()
        pollTask = Task {
            var consecutiveFailures = 0
            while !Task.isCancelled {
                try? await Task.sleep(for: .milliseconds(1500))
                guard !Task.isCancelled else { return }
                do {
                    let job = try await DaemonClient.shared.getSetupJobStatus(jobId: jobId)
                    consecutiveFailures = 0
                    await MainActor.run {
                        setupJob = job
                        progressPercent = job.overallPercent
                        if let current = job.steps.first(where: { $0.status == .inProgress }) {
                            progressMessage = current.message ?? L10n.tr("setup.progress.working")
                        }
                    }
                    if job.isTerminal {
                        await MainActor.run { self.handleJobComplete(job) }
                        return
                    }
                } catch {
                    consecutiveFailures += 1
                    logger.warning("setup.poll_failed (\(consecutiveFailures)): \(error.localizedDescription)")
                    if consecutiveFailures >= 5 {
                        await MainActor.run {
                            busy = false
                            errorMessage = L10n.tr("setup.error.lost_backend_connection")
                            step = .error
                        }
                        return
                    }
                }
            }
        }
    }

    func handleJobComplete(_ job: SetupJobState) {
        busy = false
        pollTask?.cancel()
        switch job.status {
        case .succeeded:
            apiKey = ""
            step = .captureSetup
        case .failed:
            errorMessage = job.error ?? L10n.tr("setup.error.title")
            step = .error
        case .canceled:
            errorMessage = L10n.tr("setup.error.canceled")
            step = .error
        default: break
        }
    }

    func skipCapture() {
        step = .complete
    }

    func continueFromCapture() {
        step = .complete
    }

    func completeSetup() {
        guard !isFinishingSetup else { return }
        isFinishingSetup = true
        Task {
            do {
                try await DaemonClient.shared.markOnboardingComplete()
                await MainActor.run {
                    isFinishingSetup = false
                    NotificationCenter.default.post(name: .bobeSetupCompleted, object: nil)
                }
            } catch {
                await MainActor.run {
                    isFinishingSetup = false
                    errorMessage = error.localizedDescription
                    step = .error
                }
            }
        }
    }
}
