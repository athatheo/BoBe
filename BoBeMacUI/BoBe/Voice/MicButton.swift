import AVFoundation
import OSLog
import SwiftUI

private let logger = Logger(subsystem: "com.bobe.app", category: "MicButton")

/// Mic toggle button. Tap once to connect — the daemon's Silero VAD drives
/// recording start/stop after that. While the daemon is thinking/speaking, the
/// button shows the current state and is disabled until M4.5.5 barge-in lands.
struct MicButton: View {
    @Environment(\.theme) private var theme
    @State private var pipeline = VoicePipeline.shared
    @State private var permissionStatus: AVAuthorizationStatus =
        AVCaptureDevice.authorizationStatus(for: .audio)
    /// `nil` until first poll completes. `true` when all four voice models
    /// are on disk; `false` means tapping should route to settings rather
    /// than attempt a doomed `/voice/stream` connection.
    @State private var modelsInstalled: Bool?
    /// Mirrors `DaemonSettings.voiceEnabled`. `nil` until the first settings
    /// fetch lands; `false` hides the mic + opens settings on tap.
    @State private var voiceEnabled: Bool?
    @State private var pollTask: Task<Void, Never>?

    var body: some View {
        Button(action: self.handleTap) {
            ZStack {
                // Pulsing input-level ring — only visible while the WS is
                // active. Width grows with the user's voice so they get
                // immediate feedback that the mic is being heard, BEFORE
                // the daemon's VAD/STT decides anything.
                if self.showInputRing {
                    Circle()
                        .stroke(
                            self.theme.colors.primary.opacity(0.6),
                            lineWidth: 2
                        )
                        .frame(
                            width: 36 + CGFloat(self.pipeline.inputLevel) * 24,
                            height: 36 + CGFloat(self.pipeline.inputLevel) * 24
                        )
                        .opacity(0.3 + Double(self.pipeline.inputLevel) * 0.7)
                        .animation(.easeOut(duration: 0.08), value: self.pipeline.inputLevel)
                }
                Circle()
                    .fill(self.background)
                    .frame(width: 36, height: 36)
                Image(systemName: self.icon)
                    .font(.system(size: 12, weight: .bold))
                    .foregroundStyle(self.foreground)
            }
        }
        .buttonStyle(.plain)
        .accessibilityLabel(self.accessibilityLabel)
        .help(self.tooltip)
        .frame(width: 60, height: 60)
        .contentShape(Circle())
        .disabled(self.isDisabled)
        .task {
            await self.refreshGate()
            // Pre-warm VPIO/AGC so the first 300ms-1s of speech isn't attenuated.
            // Skipped when permission isn't granted, voice models aren't
            // installed, or voice is disabled in settings.
            if self.shouldPrewarmEagerly {
                await self.pipeline.prewarm()
            }
        }
        .onDisappear { self.pollTask?.cancel() }
        .onReceive(
            NotificationCenter.default.publisher(for: .bobeWelcomeCompleted)
        ) { _ in
            // Wizard finished — install/settings state likely changed.
            Task { await self.refreshGate() }
        }
        .onReceive(
            NotificationCenter.default.publisher(for: .bobeVoiceConfigChanged)
        ) { _ in
            // Voice settings or install state changed in Settings → Voice.
            Task { await self.refreshGate() }
        }
    }

    /// Show the pulsing ring only when the daemon could plausibly be
    /// hearing audio — listening (mic open, awaiting speech) or capturing
    /// (mid-utterance). Hidden while idle / connecting / thinking / speaking.
    private var showInputRing: Bool {
        switch self.pipeline.state {
        case .listening, .capturing: true
        default: false
        }
    }

    private var needsSetup: Bool {
        // True when models are missing OR voice is explicitly disabled.
        // `nil` (unknown) is NOT needsSetup — avoids a wrench during the
        // brief boot window before the first poll lands.
        self.modelsInstalled == false || self.voiceEnabled == false
    }

    private var shouldPrewarmEagerly: Bool {
        self.permissionStatus == .authorized
            && self.modelsInstalled == true
            && self.voiceEnabled != false
    }

    private var accessibilityLabel: String {
        if self.permissionStatus == .denied || self.permissionStatus == .restricted {
            return L10n.tr("overlay.input.mic.permission_denied")
        }
        return self.needsSetup
            ? L10n.tr("overlay.input.mic.needs_setup")
            : L10n.tr("overlay.input.mic.accessibility")
    }

    private var tooltip: String {
        // Permission > setup gate > pipeline error > pipeline busy state >
        // generic toggle. Show whichever takes precedence.
        if self.permissionStatus == .denied || self.permissionStatus == .restricted {
            return L10n.tr("overlay.input.mic.permission_denied")
        }
        if self.needsSetup {
            return L10n.tr("overlay.input.mic.needs_setup")
        }
        if let err = self.pipeline.lastError, !err.isEmpty {
            return err
        }
        switch self.pipeline.state {
        case .connecting: return L10n.tr("overlay.input.mic.disabled.connecting")
        case .thinking: return L10n.tr("overlay.input.mic.disabled.thinking")
        case .speaking: return L10n.tr("overlay.input.mic.disabled.speaking")
        case .cancelling: return L10n.tr("overlay.input.mic.disabled.cancelling")
        default: return L10n.tr("overlay.input.mic.accessibility")
        }
    }

    private func handleTap() {
        // Setup gate: missing models OR voice-disabled → deep-link to the
        // Settings → Voice pane rather than attempt a /voice/stream that
        // will close with `voice_disabled` or `engines_unavailable`. Mic
        // permission is still requested first if undetermined so the UX
        // stays linear.
        if self.needsSetup {
            self.openVoiceSettings()
            return
        }
        switch self.permissionStatus {
        case .authorized:
            Task {
                guard let url = URL(string: DaemonConfig.baseURL)
                    ?? URL(string: "http://127.0.0.1:8766") else { return }
                await self.pipeline.toggle(daemonBaseURL: url)
            }
        case .notDetermined:
            Task {
                let granted = await AVCaptureDevice.requestAccess(for: .audio)
                self.permissionStatus = granted ? .authorized : .denied
                if granted, self.shouldPrewarmEagerly {
                    await self.pipeline.prewarm()
                }
            }
        case .denied, .restricted:
            if let url = URL(
                string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
            ) {
                NSWorkspace.shared.open(url)
            }
        @unknown default:
            break
        }
    }

    private func openVoiceSettings() {
        // Land directly on the Voice category — no wizard rewind.
        SettingsWindowManager.shared.show(initialCategory: .voice)
        // Re-poll after the user closes settings so the icon updates.
        self.schedulePoll()
    }

    /// Refresh both install presence and the voice-enabled toggle. The two
    /// signals are independent and either flips the wrench affordance, so
    /// they share one refresh entry point.
    private func refreshGate() async {
        async let install: VoiceInstallSnapshot? = (try? await DaemonClient.shared.voiceInstallStatus())
        async let settings: DaemonSettings? = (try? await DaemonClient.shared.getSettings())
        let (installRes, settingsRes) = await (install, settings)
        if let installRes {
            self.modelsInstalled = installRes.installed.allPresent
        } else {
            logger.debug("voice install status fetch failed")
            self.modelsInstalled = nil
        }
        if let settingsRes {
            self.voiceEnabled = settingsRes.voiceEnabled
        } else {
            logger.debug("settings fetch failed")
            self.voiceEnabled = nil
        }
    }

    /// Background poll until the gate flips. Stops once both signals are
    /// "ready" (installed=true, enabled=true) so we don't burn requests
    /// every 30s forever.
    private func schedulePoll() {
        self.pollTask?.cancel()
        self.pollTask = Task {
            for _ in 0..<60 where !Task.isCancelled {
                try? await Task.sleep(nanoseconds: 30_000_000_000)
                await self.refreshGate()
                if !self.needsSetup { break }
            }
        }
    }

    private var icon: String {
        if self.needsSetup { return "wrench.and.screwdriver" }
        if self.permissionStatus == .denied || self.permissionStatus == .restricted {
            return "mic.slash.circle.fill"
        }
        return switch self.pipeline.state {
        case .idle: "mic.slash.fill"
        case .connecting: "antenna.radiowaves.left.and.right" // distinct from .thinking
        case .thinking: "hourglass"
        case .listening: "mic.fill"
        case .capturing: "waveform.circle.fill"
        case .speaking: "waveform"
        case .cancelling: "exclamationmark.octagon.fill"
        case .failed: "exclamationmark.triangle.fill"
        }
    }

    private var background: Color {
        if self.needsSetup { return self.theme.colors.border.opacity(0.7) }
        return switch self.pipeline.state {
        case .capturing: self.theme.colors.primary
        case .speaking: self.theme.colors.secondary.opacity(0.85)
        case .listening: self.theme.colors.secondary.opacity(0.5)
        case .idle: self.theme.colors.border
        case .failed: self.theme.colors.primary.opacity(0.4)
        default: self.theme.colors.border.opacity(0.6)
        }
    }

    private var foreground: Color {
        if self.needsSetup { return self.theme.colors.textMuted }
        return switch self.pipeline.state {
        case .capturing, .speaking, .listening: self.theme.colors.background
        case .idle:
            if self.permissionStatus == .authorized {
                self.theme.colors.text
            } else {
                self.theme.colors.textMuted
            }
        default: self.theme.colors.textMuted
        }
    }

    /// Disable taps while the daemon owns the turn — until M4.5.5 barge-in,
    /// clicks would orphan an in-flight TTS playback. Always enabled when
    /// in needs-setup so the user can reach settings.
    private var isDisabled: Bool {
        if self.needsSetup { return false }
        return switch self.pipeline.state {
        case .connecting, .thinking, .speaking, .cancelling: true
        default: false
        }
    }
}
