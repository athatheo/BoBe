import SwiftUI

/// Click-driven interrupt during BoBe's reply. The mic button is disabled
/// during `.thinking` / `.speaking` (so it doesn't orphan an in-flight
/// turn), so without this button the user can only barge in by speaking
/// over the daemon's TTS output — a poor UX when they want to *stop*
/// BoBe rather than steer it.
///
/// Hidden in all other states so it doesn't clutter the composer.
struct StopButton: View {
    @State private var pipeline = VoicePipeline.shared
    @Environment(\.theme) private var theme

    var body: some View {
        if self.isVisible {
            Button(action: self.pipeline.interrupt) {
                ZStack {
                    Circle()
                        .fill(self.theme.colors.tertiary.opacity(0.85))
                        .frame(width: 36, height: 36)
                    Image(systemName: "stop.fill")
                        .font(.system(size: 12, weight: .bold))
                        .foregroundStyle(self.theme.colors.background)
                }
            }
            .buttonStyle(.plain)
            .accessibilityLabel(L10n.tr("menu.voice.stop_speaking"))
            .help("Stop BoBe (barge in)")
            .frame(width: 36, height: 36)
            .contentShape(Circle())
            .transition(.opacity.combined(with: .scale))
        }
    }

    private var isVisible: Bool {
        switch self.pipeline.state {
        case .thinking, .speaking: true
        default: false
        }
    }
}
