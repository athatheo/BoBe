import AVFoundation
import SwiftUI

/// Mic toggle button. Tap once to connect — the client-side FluidAudio
/// engine drives recording start/stop after that. While the daemon is
/// thinking/speaking, the button shows the current state.
struct MicButton: View {
    @Environment(\.theme) private var theme
    @State private var pipeline = VoicePipeline.shared
    @State private var pollTask: Task<Void, Never>?

    var body: some View {
        Button(action: self.handleTap) {
            ZStack {
                // Filled body — matches ChatToggleButton's 32pt diameter
                // so the two satellites read as a consistent pair on the
                // avatar rim.
                Circle()
                    .fill(self.background)
                    .frame(width: 32, height: 32)

                // White stroke ring just like ChatToggleButton — gives
                // the satellite a clear outer edge against the dark
                // avatar rim and any background behind the overlay.
                Circle()
                    .stroke(self.theme.colors.background, lineWidth: 2)
                    .frame(width: 32, height: 32)

                Image(systemName: self.icon)
                    .font(.system(size: 12, weight: .bold))
                    .foregroundStyle(self.foreground)
                    .opacity(self.unusableDim ? 0.45 : 1.0)
                if self.isSttBusy {
                    // Determinate progress ring around the mic — replaces the
                    // indeterminate spinner so the user can see how much of
                    // the ~940MB download has landed. Starts at 12 o'clock,
                    // sweeps clockwise as percent climbs. Pinned to a 3%
                    // minimum so the user sees an immediate sliver of motion
                    // the instant the install begins, instead of an empty
                    // stroke that looks broken.
                    let displayed = max(0.03, self.installFraction)
                    Circle()
                        .trim(from: 0, to: displayed)
                        .stroke(
                            self.theme.colors.primary,
                            style: StrokeStyle(lineWidth: 2.5, lineCap: .round)
                        )
                        .rotationEffect(.degrees(-90))
                        .frame(width: 36, height: 36)
                        .animation(.easeOut(duration: 0.25), value: displayed)
                        .accessibilityElement()
                        .accessibilityLabel(L10n.tr("overlay.input.mic.install_progress.accessibility"))
                        .accessibilityValue(
                            String(
                                format: L10n.tr("overlay.input.mic.install_progress.value_format"),
                                Int((self.installFraction * 100).rounded())
                            )
                        )
                }
                // Setup-needed badge: an action badge in the corner that
                // signals what tap will do. `arrow.down.circle.fill` for
                // "tap to download," `exclamationmark.circle.fill` for the
                // other failure modes where tap goes to settings.
                if self.needsSetup, !self.isSttBusy {
                    Image(systemName: self.badgeIcon)
                        .font(.system(size: 11))
                        .foregroundStyle(self.theme.colors.primary)
                        .background(
                            Circle()
                                .fill(self.theme.colors.background)
                                .frame(width: 13, height: 13)
                        )
                        .offset(x: 10, y: -10)
                }
            }
        }
        .buttonStyle(.plain)
        .shadow(color: self.theme.colors.text.opacity(0.10), radius: 3, y: 1)
        .accessibilityLabel(self.accessibilityLabel)
        .help(self.tooltip)
        .frame(width: 32, height: 32)
        .contentShape(Circle())
        .disabled(self.isDisabled)
        .task {
            self.pipeline.updatePermission(
                AVCaptureDevice.authorizationStatus(for: .audio)
            )
            // Pull settings first so the pipeline's activeSttLanguage is
            // current — the presence check below routes to the right engine
            // (Parakeet for English, Qwen3 for everything else).
            await self.pipeline.refreshDaemonState()
            // Filesystem presence check — cheap, doesn't need mic permission
            // or active load. Fixes a chicken-and-egg where the mic showed
            // the needs-setup badge at boot because sttStatus started as
            // .notLoaded even when the model was on disk.
            self.pipeline.bootstrapSttPresence()
            // Load STT into memory so the first audio frame after `toggle()`
            // has the model warm — but DON'T start the audio engine here.
            // Eagerly enabling VPIO activates macOS's system-wide audio
            // ducking for the whole app lifetime; we now defer that until
            // the user actually toggles the mic on. See
            // `VoicePipeline.prewarmSttOnly()` for the full rationale.
            if self.pipeline.readiness == .ready {
                await self.pipeline.prewarmSttOnly()
            }
        }
        .onDisappear { self.pollTask?.cancel() }
    }

    /// Voice can't be used right now — show the warning badge + route taps
    /// to settings/wizard rather than attempt a doomed connect. Excludes
    /// `.installing` (that gets a spinner overlay instead) and `.preparing`
    /// (transient — avoid a wrench-flicker during the boot window).
    private var needsSetup: Bool {
        switch self.pipeline.readiness {
        case .disabledByUser, .modelsMissing, .failed: true
        default: false
        }
    }

    private var accessibilityLabel: String {
        switch self.pipeline.readiness {
        case .permissionMissing:
            L10n.tr("overlay.input.mic.permission_denied")
        case .modelsMissing:
            L10n.tr("overlay.input.mic.tap_to_install")
        case .disabledByUser, .failed:
            L10n.tr("overlay.input.mic.needs_setup")
        default:
            L10n.tr("overlay.input.mic.accessibility")
        }
    }

    private var tooltip: String {
        // Readiness > pipeline error > pipeline busy state > generic toggle.
        switch self.pipeline.readiness {
        case .permissionMissing:
            return L10n.tr("overlay.input.mic.permission_denied")
        case .modelsMissing:
            return L10n.tr("overlay.input.mic.tap_to_install")
        case .disabledByUser:
            return L10n.tr("overlay.input.mic.needs_setup")
        case .installing:
            let percent = Int((self.installFraction * 100).rounded())
            return L10n.tr("overlay.input.mic.installing", percent)
        case let .failed(msg):
            return L10n.tr("overlay.input.mic.failed_format", msg)
        case .preparing, .ready:
            break
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
        // Setup gate, branched by intent:
        //  - .modelsMissing → kick off the install in place and show a
        //    determinate ring; no settings detour. The user pressed the mic
        //    to talk, not to read voice settings.
        //  - .disabledByUser or .failed → open Settings → Voice (the user
        //    needs to either re-enable voice or diagnose the failure).
        //  - .installing → open Settings → Voice so they can see per-model
        //    progress or cancel. The mic itself is already busy.
        switch self.pipeline.readiness {
        case .modelsMissing:
            self.startInstallFromMic()
            return
        case .installing:
            self.openVoiceSettings()
            return
        case .disabledByUser, .failed:
            self.openVoiceSettings()
            return
        case .permissionMissing, .preparing, .ready:
            break
        }
        switch self.pipeline.permission {
        case .authorized:
            Task {
                guard let url = URL(string: DaemonConfig.baseURL) else { return }
                await self.pipeline.toggle(daemonBaseURL: url)
            }
        case .notDetermined:
            Task {
                let granted = await AVCaptureDevice.requestAccess(for: .audio)
                self.pipeline.updatePermission(granted ? .authorized : .denied)
                if granted, self.pipeline.readiness == .ready {
                    // User granted permission but hasn't toggled the mic
                    // yet — settle STT only. Full VPIO prewarm waits for
                    // the actual toggle (see VoicePipeline.prewarmSttOnly).
                    await self.pipeline.prewarmSttOnly()
                }
            }
        case .denied, .restricted:
            if let url = URL(
                string: "x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?Privacy_Microphone"
            ) {
                NSWorkspace.shared.open(url)
            }
        @unknown default:
            break
        }
    }

    /// Kick off a daemon-side TTS install + client-side STT load in
    /// parallel. Mirrors `VoiceSetupStepView.kickoff()` so behaviour stays
    /// consistent between the wizard and the mic-button tap. Errors land
    /// on `pipeline.lastError` and the icon flips to `.failed`.
    private func startInstallFromMic() {
        Task {
            await self.pipeline.ensureSttLoaded()
        }
        Task {
            // Errors are absorbed: pipeline.lastError has no setter from
            // this scope, and the next readiness refresh flips back to
            // .modelsMissing if install didn't take.
            try? await DaemonClient.shared.startVoiceInstall()
            // Poll until install resolves so the badge/icon picks up the
            // state change without waiting on the user's next interaction.
            self.schedulePoll()
        }
    }

    private func openVoiceSettings() {
        // Land directly on the Voice category — no wizard rewind.
        SettingsWindowManager.shared.show(initialCategory: .voice)
        // Re-poll after the user closes settings so the icon updates.
        self.schedulePoll()
    }

    /// Background poll until the gate flips. Stops once readiness reaches
    /// `.ready` so we don't burn requests every 30s forever. Also short-
    /// circuits on `.failed` — that's a terminal state until the user takes
    /// action, polling it 60 more times changes nothing.
    private func schedulePoll() {
        self.pollTask?.cancel()
        self.pollTask = Task {
            for _ in 0 ..< 60 where !Task.isCancelled {
                try? await Task.sleep(for: .seconds(30))
                await self.pipeline.refreshDaemonState()
                if !self.needsSetup { break }
                if case .failed = self.pipeline.readiness { break }
            }
        }
    }

    private var icon: String {
        // The MicButton is now a quiet on/off toggle — live voice activity
        // lives on the avatar's presence ring. Four intents:
        //   - permission missing → mic.slash.circle.fill (call out the block)
        //   - needs setup (models missing) → wrench.and.screwdriver.fill
        //     (mic.slash misleads — it implies "muted", not "configure me")
        //   - mic ON (open or mid-turn) → mic.fill
        //   - mic OFF (idle/failed but ready) → mic.slash.fill (the real
        //     "muted" meaning)
        if self.pipeline.readiness == .permissionMissing {
            return "mic.slash.circle.fill"
        }
        if self.needsSetup { return "wrench.and.screwdriver.fill" }
        return switch self.pipeline.state {
        case .idle, .failed: "mic.slash.fill"
        case .connecting, .thinking, .speaking, .cancelling, .listening, .capturing: "mic.fill"
        }
    }

    /// True only while an active download is running (either Kokoro daemon-
    /// side or Parakeet client-side). Drives the spinner overlay; distinct
    /// from `needsSetup` which routes taps to the wizard.
    private var isSttBusy: Bool {
        self.pipeline.readiness == .installing
    }

    /// Whether the mic icon should render with reduced opacity (signals
    /// "voice unavailable" while keeping the icon recognizable as a mic).
    private var unusableDim: Bool {
        switch self.pipeline.readiness {
        case .ready, .preparing: false
        case .disabledByUser, .permissionMissing, .installing, .modelsMissing, .failed: true
        }
    }

    private var background: Color {
        if self.needsSetup { return self.theme.colors.border.opacity(0.7) }
        return switch self.pipeline.state {
        // ON — mic is open and audio is flowing. The listening vs capturing
        // distinction now lives entirely on the avatar's turbulent presence
        // ring; the mic itself is a binary toggle.
        case .listening, .capturing: self.theme.colors.primary
        case .speaking, .thinking: self.theme.colors.secondary.opacity(0.55)
        case .idle: self.theme.colors.border
        case .failed: self.theme.colors.primary.opacity(0.4)
        case .connecting, .cancelling: self.theme.colors.border.opacity(0.6)
        }
    }

    private var foreground: Color {
        if self.needsSetup { return self.theme.colors.textMuted }
        return switch self.pipeline.state {
        case .capturing, .speaking, .listening, .thinking: self.theme.colors.background
        case .idle:
            if self.pipeline.permission == .authorized {
                self.theme.colors.text
            } else {
                self.theme.colors.textMuted
            }
        case .connecting, .cancelling, .failed: self.theme.colors.textMuted
        }
    }

    /// Disable taps while the daemon owns the turn — taps go through the
    /// stop-button affordance for explicit cancellation. Always enabled
    /// while needs-setup or installing so the user can reach the install
    /// flow / settings.
    private var isDisabled: Bool {
        switch self.pipeline.readiness {
        case .ready: break
        case .disabledByUser, .modelsMissing, .failed, .permissionMissing, .installing: return false
        case .preparing: return true
        }
        return switch self.pipeline.state {
        case .connecting, .thinking, .speaking, .cancelling: true
        default: false
        }
    }

    /// Aggregate install progress as a 0…1 fraction. Picks the lowest of
    /// (daemon TTS percent, client STT presence) so the ring only reaches
    /// 100% when *both* halves have landed. STT is binary (ready / not),
    /// so it contributes 0 or 1; TTS reports an actual percent.
    private var installFraction: CGFloat {
        let ttsPercent: Double = {
            guard let models = self.pipeline.installSnapshot?.models else { return 0 }
            let percents = models.compactMap(\.percent)
            guard !percents.isEmpty else { return 0 }
            return Double(percents.reduce(0, +)) / Double(percents.count) / 100.0
        }()
        let sttFraction: Double = (self.pipeline.sttStatus == .ready) ? 1.0 : 0.0
        // Show the slower side so the ring doesn't overshoot reality. While
        // STT is downloading (we have no percent for it) the ring is capped
        // at the TTS percent or 0.05 (just enough to show "started").
        let combined = min(ttsPercent, sttFraction)
        return CGFloat(max(combined, ttsPercent > 0 ? ttsPercent * 0.5 : 0.04))
    }

    /// Icon for the corner badge. `arrow.down.circle.fill` reads as "tap
    /// to download" for modelsMissing; the warning glyph stays for the
    /// other failure modes where tap goes to settings.
    private var badgeIcon: String {
        switch self.pipeline.readiness {
        case .modelsMissing: "arrow.down.circle.fill"
        default: "exclamationmark.circle.fill"
        }
    }
}
