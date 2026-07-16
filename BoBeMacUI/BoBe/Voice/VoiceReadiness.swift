import AVFoundation
import Foundation

/// Aggregated voice readiness — single signal for MicButton / wizard /
/// banners. Priority: disabledByUser > permissionMissing > preparing >
/// failed > installing > modelsMissing > ready. Earlier short-circuits.
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
    /// → Parakeet; everything else → Nemotron multilingual.
    var activeStt: any VoiceSttEngine {
        self.sttEngine(for: self.effectiveLanguage)
    }

    func sttEngine(for language: String) -> any VoiceSttEngine {
        language == "en" ? self.parakeetStt : self.nemotronStt
    }

    /// Presence-check the active language's model bundle. Mirrors the
    /// `activeStt` switch so the readiness check matches what
    /// `ensureSttLoaded()` will load.
    var activeModelIsInstalled: Bool {
        self.modelIsInstalled(for: self.effectiveLanguage)
    }

    func modelIsInstalled(for language: String) -> Bool {
        switch language {
        case "en": FluidAudioModelPresence.isInstalled()
        default: FluidAudioNemotronModelPresence.isInstalled()
        }
    }

    /// Aggregated readiness folded from the four underlying observable
    /// inputs. See `VoiceReadiness` for case priority.
    public var readiness: VoiceReadiness {
        if self.voiceEnabled == false {
            return .disabledByUser
        }
        if self.permission == .denied || self.permission == .restricted {
            return .permissionMissing
        }
        guard let snapshot = self.installSnapshot, self.voiceEnabled == true else {
            return .preparing
        }
        if case let .failed(msg) = self.sttStatus {
            return .failed(msg)
        }
        // `clientSttStatus` here is whichever engine the active language
        // routes to (Parakeet for English, Nemotron for everything else) —
        // both engines write through to `sttStatus`.
        let daemonTtsDownloading = snapshot.isRunning
            && VoiceTtsPreference.shared.backend == .serverKokoro
        let selectedTtsReady = switch VoiceTtsPreference.shared.backend {
        case .serverKokoro: snapshot.installed.tts
        case .clientSupertonic: FluidAudioSupertonicModelPresence.isInstalled()
        }
        let clientSttDownloading = self.sttStatus == .downloading
        let clientSttReady = self.sttStatus == .ready
        if daemonTtsDownloading || clientSttDownloading {
            return .installing
        }
        if selectedTtsReady, clientSttReady {
            return .ready
        }
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
                self.applySettings(settingsRes)
            }
        }
        self.refreshDaemonTask = task
        await task.value
        self.refreshDaemonTask = nil
    }

    func applySettings(_ settings: DaemonSettings) {
        self.voiceEnabled = settings.voiceEnabled
        self.showPartialCaption = settings.voiceShowPartialCaption
        self.voicePersona = settings.voicePersona
        self.voiceSpeed = settings.voiceSpeed
        let language = settings.voiceSttLanguage
        if !language.isEmpty, language != self.activeSttLanguage {
            self.activeSttLanguage = language
            self.sttStatus = self.loadedLanguages.contains(language)
                || self.modelIsInstalled(for: language) ? .ready : .notLoaded
        }
        let delayMs = Self.eouDelayMs(for: settings.voicePauseSensitivity)
        let nemotron = self.nemotronStt
        let parakeet = self.parakeetStt
        Task {
            await nemotron.setEouDelayMs(delayMs)
            await parakeet.setEouDebounceMs(delayMs)
        }
        if !settings.voiceEnabled, self.state != .idle {
            self.disconnect()
        }
    }

    /// Map the daemon's `voice.pause_sensitivity` string to an EOU debounce
    /// (ms). Same value flows to both engines: Parakeet's built-in EOU
    /// debounce, and the Nemotron wrapper's VAD-driven silence timer.
    /// Concrete values live in `Constants.PauseSensitivityMs` so the drift
    /// script can lock them to Rust `constants::pause_sensitivity_ms::*`.
    /// The 800ms balanced default trims ~480ms vs. Parakeet's published
    /// 1280ms reference — the model's linguistic EOU prediction is good
    /// enough that we don't need a full second of trailing silence to be
    /// confident the user is done.
    static func eouDelayMs(for sensitivity: String) -> Int {
        switch sensitivity.lowercased() {
        case "tight": PauseSensitivityMs.tight
        case "patient": PauseSensitivityMs.patient
        default: PauseSensitivityMs.balanced
        }
    }

    /// Mic-permission writeback. `MicButton` calls this after
    /// `AVCaptureDevice.requestAccess` resolves so `readiness` picks up the
    /// new value and other consumers stay in sync.
    public func updatePermission(_ status: AVAuthorizationStatus) {
        self.permission = status
    }
}
