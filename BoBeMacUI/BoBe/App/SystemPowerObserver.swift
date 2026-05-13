import AppKit
import OSLog

private let logger = Logger(subsystem: "com.bobe.app", category: "SystemPower")

/// Observes `NSWorkspace` sleep/wake notifications and routes them to
/// `VoicePipeline` so the always-on capture/playback engines don't keep
/// running across a long sleep (mic indicator stays on, the realtime tap
/// queue can accumulate buffers, AGC is wedged on resume).
///
/// `BobeStore.registerSleepWakeObservers` already handles SSE reconnect
/// on wake; this is the parallel for the voice subsystem.
@MainActor
final class SystemPowerObserver {
    static let shared = SystemPowerObserver()

    private var sleepObserver: NSObjectProtocol?
    private var wakeObserver: NSObjectProtocol?

    private init() {}

    /// Idempotent — safe to call multiple times during app launch.
    func start() {
        let center = NSWorkspace.shared.notificationCenter
        if self.sleepObserver == nil {
            self.sleepObserver = center.addObserver(
                forName: NSWorkspace.willSleepNotification,
                object: nil,
                queue: .main
            ) { _ in
                Task { @MainActor in
                    logger.info("System will sleep — disconnecting voice pipeline")
                    // `disconnect()` already tears down the audio engines
                    // and clears `isWarm` so the next prewarm restarts cleanly.
                    VoicePipeline.shared.disconnect()
                }
            }
        }
        if self.wakeObserver == nil {
            self.wakeObserver = center.addObserver(
                forName: NSWorkspace.didWakeNotification,
                object: nil,
                queue: .main
            ) { _ in
                Task { @MainActor in
                    // Re-prewarm only if the user previously had voice running
                    // (i.e., MicButton's prewarm path will run on next overlay
                    // appear; we don't force it here to respect the muted state).
                    logger.info("System did wake — voice pipeline will rewarm on next interaction")
                }
            }
        }
    }

    func stop() {
        let center = NSWorkspace.shared.notificationCenter
        if let s = self.sleepObserver { center.removeObserver(s); self.sleepObserver = nil }
        if let w = self.wakeObserver { center.removeObserver(w); self.wakeObserver = nil }
    }
}
