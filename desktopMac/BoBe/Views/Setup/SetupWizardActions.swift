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
        guard !busy, !apiKey.trimmingCharacters(in: .whitespaces).isEmpty else { return }
        let provider = options?.cloudProviders.first { $0.id == selectedProvider }
        if provider?.needsEndpoint == true, endpoint.trimmingCharacters(in: .whitespaces).isEmpty { return }
        if provider?.needsDeployment == true, deployment.trimmingCharacters(in: .whitespaces).isEmpty { return }

        busy = true
        setupMode = .online
        var request = SetupRequest(mode: "cloud")
        request.provider = selectedProvider
        request.apiKey = apiKey
        request.model = selectedModel.isEmpty ? nil : selectedModel
        if provider?.needsEndpoint == true { request.endpoint = endpoint }
        if provider?.needsDeployment == true { request.deployment = deployment }
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
        progressMessage = "Starting setup..."
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
                            progressMessage = current.message ?? "Working..."
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
                            errorMessage = "Lost connection to the backend during setup. Please try again."
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
            errorMessage = job.error ?? "Setup failed"
            step = .error
        case .canceled:
            errorMessage = "Setup was canceled"
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
