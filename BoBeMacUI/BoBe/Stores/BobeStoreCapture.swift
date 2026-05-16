import CoreGraphics
import Foundation

extension BobeStore {
    func synchronizeCaptureStartup() {
        self.captureStartupTask?.cancel()
        self.captureStartupTask = Task { @MainActor [weak self] in
            guard let self else { return }
            do {
                let settings = try await client.getSettings()

                guard settings.captureEnabled else {
                    self.updateState { ctx in
                        ctx.capturing = false
                        ctx.captureInProgress = false
                    }
                    return
                }

                var captureActive = false
                for attempt in 0 ..< 3 where !Task.isCancelled {
                    do {
                        try await self.client.startCapture()
                        captureActive = true
                        break
                    } catch let DaemonError.httpError(statusCode, message)
                        where statusCode == 409 || message.localizedCaseInsensitiveContains("already") {
                        captureActive = true
                        break
                    } catch {
                        bobeStoreLogger.warning("Capture startup sync attempt \(attempt + 1) failed: \(error.localizedDescription)")
                        if attempt < 2 {
                            try? await Task.sleep(for: .milliseconds(StoreTiming.captureRetryBaseMilliseconds * (attempt + 1)))
                        }
                    }
                }

                if !captureActive, !CGPreflightScreenCaptureAccess() {
                    CGRequestScreenCaptureAccess()
                    self.updateState { $0.capturePermissionMissing = true }
                    bobeStoreLogger.warning("Screen capture permission not granted")
                } else {
                    self.updateState { $0.capturePermissionMissing = false }
                }
                self.updateState { ctx in
                    ctx.capturing = captureActive
                    ctx.captureInProgress = false
                }
            } catch {
                bobeStoreLogger.warning("Capture startup sync skipped: \(error.localizedDescription)")
            }
        }
    }

    func handleConnectionChange(_ connected: Bool) {
        self.updateState { ctx in
            ctx.daemonConnected = connected
            if connected { ctx.daemonError = false }
        }

        if connected {
            self.cancelReconnectStatusTransition()
            self.isReconnecting = false
            self.hasConnectedOnce = true
            self.synchronizeCaptureStartup()
            self.synchronizeStatus()
            return
        }

        guard self.hasConnectedOnce else { return }
        self.scheduleReconnectStatusTransition()
    }

    /// Without this, mid-turn SSE reconnects let the composer re-enable too early and 409.
    private func synchronizeStatus() {
        Task { @MainActor [weak self] in
            guard let self else { return }
            do {
                let status = try await self.client.getStatus()
                self.updateState { ctx in
                    ctx.acceptingUserMessages = status.acceptingUserMessages
                    let indicator = status.indicatorType
                    ctx.thinking = indicator == .thinking
                    ctx.speaking = indicator == .streaming
                    ctx.captureInProgress = indicator == .screenCapture
                }
            } catch {
                bobeStoreLogger.warning("Status sync skipped: \(error.localizedDescription)")
            }
        }
    }

    func scheduleReconnectStatusTransition() {
        self.reconnectStatusTask?.cancel()
        self.reconnectStatusTask = Task { @MainActor [weak self] in
            try? await Task.sleep(for: .milliseconds(StoreTiming.reconnectStatusDelayMilliseconds))
            guard let self, !Task.isCancelled, !self.context.daemonConnected else { return }
            self.isReconnecting = true
        }
    }

    func cancelReconnectStatusTransition() {
        self.reconnectStatusTask?.cancel()
        self.reconnectStatusTask = nil
    }
}
