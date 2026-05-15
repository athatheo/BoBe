import AVFoundation
import Foundation

/// Aggregated voice readiness — the single signal consumers (MicButton,
/// wizard-reprompt logic, future banners) should switch on instead of
/// reading four separate inputs (`sttStatus` + `permission` +
/// `installSnapshot` + `voiceEnabled`).
///
/// Priority order, highest first — earlier cases short-circuit later checks:
///   1. voice disabled in settings → `.disabledByUser`
///   2. mic permission denied/restricted → `.permissionMissing`
///   3. no daemon snapshot or settings fetch yet → `.preparing`
///   4. STT load failed → `.failed`
///   5. either side actively downloading → `.installing`
///   6. either side missing on disk → `.modelsMissing`
///   7. all four green → `.ready`
public enum VoiceReadiness: Equatable {
    case preparing
    case disabledByUser
    case permissionMissing
    case installing
    case modelsMissing
    case failed(String)
    case ready
}

@MainActor
extension VoicePipeline {
    /// Effective language right now — `sessionSttLanguage` while a WS is
    /// open (locked at connect time), otherwise `activeSttLanguage` (live
    /// from settings).
    var effectiveLanguage: String {
        self.sessionSttLanguage ?? self.activeSttLanguage
    }

    /// Resolve the active STT engine from the effective language. English
    /// → Parakeet; everything else → Qwen3 (which is multilingual).
    var activeStt: any VoiceSttEngine {
        switch self.effectiveLanguage {
        case "en": return self.parakeetStt
        default: return self.qwen3Stt
        }
    }

    /// Presence-check the active language's model bundle. Mirrors the
    /// `activeStt` switch so the readiness check matches what
    /// `ensureSttLoaded()` will load.
    var activeModelIsInstalled: Bool {
        switch self.effectiveLanguage {
        case "en": return FluidAudioModelPresence.isInstalled()
        default: return FluidAudioQwen3ModelPresence.isInstalled()
        }
    }

    /// Aggregated readiness folded from the four underlying observable
    /// inputs. See `VoiceReadiness` for case priority.
    public var readiness: VoiceReadiness {
        if self.voiceEnabled == false { return .disabledByUser }
        if self.permission == .denied || self.permission == .restricted {
            return .permissionMissing
        }
        guard let snapshot = self.installSnapshot, self.voiceEnabled == true else {
            return .preparing
        }
        if case let .failed(msg) = self.sttStatus { return .failed(msg) }
        // `clientSttStatus` here is whichever engine the active language
        // routes to (Parakeet for English, Qwen3 for everything else) —
        // both engines write through to `sttStatus`.
        let daemonTtsDownloading = snapshot.isRunning
        let daemonTtsReady = snapshot.installed.allPresent
        let clientSttDownloading = self.sttStatus == .downloading
        let clientSttReady = self.sttStatus == .ready
        if daemonTtsDownloading || clientSttDownloading { return .installing }
        if daemonTtsReady && clientSttReady { return .ready }
        return .modelsMissing
    }

    /// Fetch daemon-side voice state (install snapshot + voiceEnabled toggle
    /// + language + pause sensitivity) and publish to the observable.
    /// Idempotent + deduped — concurrent callers share a single in-flight
    /// request so view-appear stampedes don't double-hit the daemon.
    public func refreshDaemonState() async {
        if let existing = self.refreshDaemonTask {
            await existing.value
            return
        }
        let task = Task { @MainActor in
            async let install: VoiceInstallSnapshot? =
                (try? await DaemonClient.shared.voiceInstallStatus())
            async let settings: DaemonSettings? =
                (try? await DaemonClient.shared.getSettings())
            let (installRes, settingsRes) = await (install, settings)
            if installRes != nil {
                self.installSnapshot = installRes
            }
            if let settingsRes {
                self.voiceEnabled = settingsRes.voiceEnabled
                let lang = settingsRes.voiceSttLanguage
                if !lang.isEmpty {
                    self.activeSttLanguage = lang
                }
                let delayMs = Self.eouDelayMs(for: settingsRes.voicePauseSensitivity)
                let qwen3 = self.qwen3Stt
                Task { await qwen3.setEouDelayMs(delayMs) }
            }
        }
        self.refreshDaemonTask = task
        await task.value
        self.refreshDaemonTask = nil
    }

    /// Map the daemon's `voice.pause_sensitivity` string to an EOU delay
    /// (ms) that the Qwen3 VAD timer waits after speechEnd before firing.
    /// Parakeet ignores this — its EOU is built into the streaming manager.
    static func eouDelayMs(for sensitivity: String) -> Int {
        switch sensitivity.lowercased() {
        case "tight": return 600
        case "patient": return 1_200
        default: return 800
        }
    }

    /// Mic-permission writeback. `MicButton` calls this after
    /// `AVCaptureDevice.requestAccess` resolves so `readiness` picks up the
    /// new value and other consumers stay in sync.
    public func updatePermission(_ status: AVAuthorizationStatus) {
        self.permission = status
    }
}
