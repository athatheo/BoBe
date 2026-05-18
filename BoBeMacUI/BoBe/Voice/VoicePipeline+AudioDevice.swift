@preconcurrency import AudioToolbox
import Foundation
import OSLog

private let logger = Logger(subsystem: "com.bobe.app", category: "VoicePipeline.AudioDevice")

/// CoreAudio integration: observe the system default-input and default-
/// output devices and rebuild the AVAudioEngine when either changes.
///
/// **Why this matters — input side.** `AVAudioEngine.inputNode.inputFormat`
/// captures the device's sample rate / channel count / interleaving at
/// the moment `installTap(...)` is called. When the user plugs in AirPods
/// (or any USB mic, or switches default input in System Settings), the
/// engine itself stays running on the OLD format — buffers arrive in the
/// new format but our `AVAudioConverter` was built for the old one.
/// Symptom: "I plugged in headphones and BoBe stopped hearing me."
///
/// **Why this matters — output side.** VPIO (the AEC AudioUnit) needs the
/// output reference signal to cancel BoBe's own voice from the mic input.
/// When the default output changes, the cached reference path in VPIO
/// can degrade AEC quality. Less critical than input (TTS still plays
/// through the new device automatically) but the AEC matters for the
/// next user turn after the route change.
///
/// **Approach.** Two CoreAudio property listeners — one for input, one
/// for output. Both debounce (HAL fires 2-3x per plug/unplug) and route
/// to the same rebuild path. Output rebuilds are DEFERRED while TTS is
/// audibly playing, because tearing down the engine mid-utterance would
/// cut audio. Deferred rebuilds drain at the next `.listening` transition.
@MainActor
extension VoicePipeline {
    // MARK: - Registration

    /// Idempotent registration of both CoreAudio listeners. Called from
    /// `configureEngine()` so the listeners are wired up as soon as we
    /// have a running engine. The listener blocks are process-lifetime —
    /// we never deregister, matching the singleton's lifetime.
    func registerInputDeviceObserverIfNeeded() {
        self.registerDeviceListener(
            selector: kAudioHardwarePropertyDefaultInputDevice,
            kind: .input,
            alreadyRegistered: self.inputDeviceListenerRegistered,
            markRegistered: { self.inputDeviceListenerRegistered = true }
        )
        self.registerDeviceListener(
            selector: kAudioHardwarePropertyDefaultOutputDevice,
            kind: .output,
            alreadyRegistered: self.outputDeviceListenerRegistered,
            markRegistered: { self.outputDeviceListenerRegistered = true }
        )
    }

    /// Distinguishes input vs output in log + behaviour. Input rebuilds
    /// fire immediately. Output rebuilds defer through active TTS.
    private enum DeviceKind: String {
        case input, output
    }

    private func registerDeviceListener(
        selector: AudioObjectPropertySelector,
        kind: DeviceKind,
        alreadyRegistered: Bool,
        markRegistered: @escaping @MainActor () -> Void
    ) {
        guard !alreadyRegistered else { return }
        var address = AudioObjectPropertyAddress(
            mSelector: selector,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )
        // Listener runs on the global concurrent queue per Apple's docs —
        // we explicitly hop to MainActor for state mutation. The block is
        // `@Sendable` to satisfy strict-concurrency checks; it captures
        // only `weak self` (the pipeline singleton).
        let status = AudioObjectAddPropertyListenerBlock(
            AudioObjectID(kAudioObjectSystemObject),
            &address,
            DispatchQueue.global(qos: .userInitiated)
        ) { [weak self] _, _ in
            Task { @MainActor [weak self] in
                self?.handleDefaultDeviceChange(kind: kind)
            }
        }
        if status == noErr {
            markRegistered()
            logger.info("voice: default-\(kind.rawValue, privacy: .public) device listener registered")
        } else {
            logger.warning(
                "voice: \(kind.rawValue, privacy: .public) device listener registration failed (status=\(status))"
            )
        }
    }

    // MARK: - Change handling

    /// Coalesce HAL device-change notifications and schedule one rebuild.
    /// The 250 ms debounce window is the empirical "settle time" for HAL
    /// device transitions — shorter and you race the format-change phase;
    /// longer and the user notices a gap mid-utterance.
    ///
    /// Output changes during active TTS are deferred via the
    /// `pendingOutputDeviceRebuild` flag — the next `.listening` phase
    /// drains them. Input changes always rebuild immediately because the
    /// input path is silent until the next user utterance regardless.
    private func handleDefaultDeviceChange(kind: DeviceKind) {
        logger.info("voice: default-\(kind.rawValue, privacy: .public) device changed")
        if kind == .output, self.audioStillPlaying() {
            // Defer — tearing down the engine now would cut TTS mid-word.
            logger.info("voice: deferring output-device rebuild — audio still playing")
            self.pendingOutputDeviceRebuild = true
            return
        }
        self.scheduleDeviceChangeRebuild()
    }

    /// Called by `applyPhase(.listening)` (in VoicePipeline.swift) when a
    /// turn finishes. If an output-device change arrived during TTS, we
    /// drain it now that the player is idle.
    func drainPendingOutputDeviceRebuild() {
        guard self.pendingOutputDeviceRebuild else { return }
        self.pendingOutputDeviceRebuild = false
        logger.info("voice: draining deferred output-device rebuild")
        self.scheduleDeviceChangeRebuild()
    }

    /// Common scheduling path. Cancels prior pending rebuilds and starts
    /// the 250 ms debounce.
    private func scheduleDeviceChangeRebuild() {
        self.deviceChangeReconfigureTask?.cancel()
        self.deviceChangeReconfigureTask = Task { @MainActor [weak self] in
            try? await Task.sleep(for: .milliseconds(250))
            guard let self, !Task.isCancelled else { return }
            self.rebuildAudioEngineForDeviceChange()
            self.deviceChangeReconfigureTask = nil
        }
    }

    /// Tear down + reconfigure the audio path in place. Does NOT touch
    /// the WebSocket — the session keeps its turn id and the daemon
    /// never notices. After the rebuild, the new device's format flows
    /// through `installTap` to the FluidAudio engines, the converter is
    /// built fresh against the new input format, and VPIO re-references
    /// against the new output.
    ///
    /// No-op when the engine isn't warm (idle session, mic off) — the
    /// next user-initiated `prewarm()` picks up the new device anyway.
    private func rebuildAudioEngineForDeviceChange() {
        guard self.isWarm else {
            logger.debug("voice: device change while cold — skipping rebuild")
            return
        }
        logger.info("voice: rebuilding audio engine for new default device")
        self.tearDownAudio()
        do {
            try self.configureEngine()
            self.isWarm = true
        } catch {
            self.lastError = "voice device-change rebuild: \(error.localizedDescription)"
            logger.error(
                "voice: engine rebuild after device change failed: \(error.localizedDescription, privacy: .public)"
            )
        }
    }
}
