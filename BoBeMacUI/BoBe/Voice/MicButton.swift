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
                    .opacity(self.unusableDim ? 0.45 : 1.0)
                if self.isSttBusy {
                    ProgressView()
                        .progressViewStyle(.circular)
                        .controlSize(.small)
                        .scaleEffect(0.7)
                }
                // Setup-needed badge: a small warning glyph in the corner
                // that stays mic-icon-anchored so the user still knows this
                // is the voice button. Doesn't render during downloads —
                // that's a spinner overlay (above).
                if self.needsSetup && !self.isSttBusy {
                    Image(systemName: "exclamationmark.circle.fill")
                        .font(.system(size: 12))
                        .foregroundStyle(self.theme.colors.primary)
                        .background(
                            Circle()
                                .fill(self.theme.colors.background)
                                .frame(width: 14, height: 14)
                        )
                        .offset(x: 11, y: -11)
                }
            }
        }
        .buttonStyle(.plain)
        .accessibilityLabel(self.accessibilityLabel)
        .help(self.tooltip)
        .frame(width: 60, height: 60)
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
            // Pre-warm VPIO/AGC so the first 300ms-1s of speech isn't attenuated.
            // Skipped when permission isn't granted, voice models aren't
            // installed, or voice is disabled in settings.
            if self.pipeline.readiness == .ready {
                await self.pipeline.prewarm()
            }
        }
        .onDisappear { self.pollTask?.cancel() }
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
            return L10n.tr("overlay.input.mic.permission_denied")
        case .disabledByUser, .modelsMissing, .failed:
            return L10n.tr("overlay.input.mic.needs_setup")
        default:
            return L10n.tr("overlay.input.mic.accessibility")
        }
    }

    private var tooltip: String {
        // Readiness > pipeline error > pipeline busy state > generic toggle.
        switch self.pipeline.readiness {
        case .permissionMissing:
            return L10n.tr("overlay.input.mic.permission_denied")
        case .disabledByUser, .modelsMissing:
            return L10n.tr("overlay.input.mic.needs_setup")
        case .installing:
            return "Downloading voice model… (~600MB, first run only)"
        case .failed(let msg):
            return "Voice model failed to load: \(msg). Tap to retry."
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
        // Setup gate: missing models OR voice-disabled → deep-link to the
        // Settings → Voice pane rather than attempt a /voice/stream that
        // will close with `voice_disabled` or `engines_unavailable`. Mic
        // permission is still requested first if undetermined so the UX
        // stays linear.
        if self.needsSetup {
            self.openVoiceSettings()
            return
        }
        switch self.pipeline.permission {
        case .authorized:
            Task {
                guard let url = URL(string: DaemonConfig.baseURL)
                    ?? URL(string: "http://127.0.0.1:8766") else { return }
                await self.pipeline.toggle(daemonBaseURL: url)
            }
        case .notDetermined:
            Task {
                let granted = await AVCaptureDevice.requestAccess(for: .audio)
                self.pipeline.updatePermission(granted ? .authorized : .denied)
                if granted, self.pipeline.readiness == .ready {
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

    /// Background poll until the gate flips. Stops once readiness reaches
    /// `.ready` so we don't burn requests every 30s forever.
    private func schedulePoll() {
        self.pollTask?.cancel()
        self.pollTask = Task {
            for _ in 0..<60 where !Task.isCancelled {
                try? await Task.sleep(nanoseconds: 30_000_000_000)
                await self.pipeline.refreshDaemonState()
                if !self.needsSetup { break }
            }
        }
    }

    private var icon: String {
        // Icon ALWAYS reads as a mic (or a mic-state variant). When voice
        // can't be used, the button stays mic-shaped but dims + shows a
        // small warning badge; the tooltip + tap routing explain what's
        // wrong. The wrench-icon-swap was confusing — users couldn't tell
        // the button was still about voice.
        if self.pipeline.readiness == .permissionMissing {
            return "mic.slash.circle.fill"
        }
        if self.needsSetup { return "mic.fill" } // mic-shaped, dimmed via opacity below
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
            if self.pipeline.permission == .authorized {
                self.theme.colors.text
            } else {
                self.theme.colors.textMuted
            }
        default: self.theme.colors.textMuted
        }
    }

    /// Disable taps while the daemon owns the turn — taps go through the
    /// stop-button affordance for explicit cancellation. Always enabled when
    /// in needs-setup so the user can reach settings. Also disabled while
    /// either model is downloading (~600MB first run) — tapping would
    /// either no-op or trigger a redundant download attempt.
    private var isDisabled: Bool {
        switch self.pipeline.readiness {
        case .ready: break
        case .disabledByUser, .modelsMissing, .failed, .permissionMissing: return false
        case .installing, .preparing: return true
        }
        return switch self.pipeline.state {
        case .connecting, .thinking, .speaking, .cancelling: true
        default: false
        }
    }
}
