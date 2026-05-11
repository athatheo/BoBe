import AVFoundation
import SwiftUI

/// Mic toggle button. Tap once to connect — the daemon's Silero VAD drives
/// recording start/stop after that. While the daemon is thinking/speaking, the
/// button shows the current state and is disabled until M4.5.5 barge-in lands.
struct MicButton: View {
    @Environment(\.theme) private var theme
    @State private var pipeline = VoicePipeline.shared
    @State private var permissionStatus: AVAuthorizationStatus =
        AVCaptureDevice.authorizationStatus(for: .audio)

    var body: some View {
        Button(action: self.handleTap) {
            ZStack {
                Circle()
                    .fill(self.background)
                    .frame(width: 36, height: 36)
                Image(systemName: self.icon)
                    .font(.system(size: 12, weight: .bold))
                    .foregroundStyle(self.foreground)
            }
        }
        .buttonStyle(.plain)
        .accessibilityLabel("Toggle voice")
        .frame(width: 36, height: 36)
        .contentShape(Circle())
        .disabled(self.isDisabled)
        .task {
            // Pre-warm VPIO/AGC so the first 300ms-1s of speech isn't attenuated.
            // Skipped when permission isn't granted; the tap flow triggers the
            // permission prompt then.
            if self.permissionStatus == .authorized {
                await self.pipeline.prewarm()
            }
        }
    }

    private func handleTap() {
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
                if granted {
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

    private var icon: String {
        switch self.pipeline.state {
        case .idle: "mic.slash.fill"
        case .connecting, .thinking: "hourglass"
        case .listening: "mic.fill"
        case .capturing: "waveform.circle.fill"
        case .speaking: "waveform"
        case .cancelling: "exclamationmark.octagon.fill"
        case .failed: "exclamationmark.triangle.fill"
        }
    }

    private var background: Color {
        switch self.pipeline.state {
        case .capturing: self.theme.colors.primary
        case .speaking: self.theme.colors.secondary.opacity(0.85)
        case .listening: self.theme.colors.secondary.opacity(0.5)
        case .idle: self.theme.colors.border
        case .failed: self.theme.colors.primary.opacity(0.4)
        default: self.theme.colors.border.opacity(0.6)
        }
    }

    private var foreground: Color {
        switch self.pipeline.state {
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
    /// clicks would orphan an in-flight TTS playback.
    private var isDisabled: Bool {
        switch self.pipeline.state {
        case .connecting, .thinking, .speaking, .cancelling: true
        default: false
        }
    }
}
