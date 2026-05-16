import SwiftUI

/// Live caption bound to `VoicePipeline.partialTranscript`. Empty → no
/// layout space; fades in/out. Gated by `showPartialCaption` so Settings
/// → Voice can hide it without affecting voice itself.
struct VoicePartialCaption: View {
    @State private var pipeline = VoicePipeline.shared
    @Environment(\.theme) private var theme

    var body: some View {
        if self.pipeline.showPartialCaption, !self.pipeline.partialTranscript.isEmpty {
            HStack(spacing: 6) {
                Image(systemName: "ear")
                    .font(.system(size: 10))
                    .foregroundStyle(self.theme.colors.primary)
                Text(self.pipeline.partialTranscript)
                    .bobeTextStyle(.helper)
                    .foregroundStyle(self.theme.colors.text)
                    .lineLimit(2)
                    .multilineTextAlignment(.leading)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(
                RoundedRectangle(cornerRadius: 8)
                    .fill(self.theme.colors.surface.opacity(0.9))
                    .overlay(
                        RoundedRectangle(cornerRadius: 8)
                            .stroke(self.theme.colors.primary.opacity(0.4), lineWidth: 1)
                    )
            )
            .transition(.opacity.combined(with: .move(edge: .bottom)))
            .animation(.easeOut(duration: 0.15), value: self.pipeline.partialTranscript)
        }
    }
}
