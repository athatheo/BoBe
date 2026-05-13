import SwiftUI

/// Shows what BoBe is hearing in near-real-time. Bound to
/// `VoicePipeline.partialTranscript` which is set on every TranscriptPartial
/// frame from the daemon's streaming Zipformer STT and cleared at turn-end.
///
/// Renders nothing while empty so it doesn't take layout space when the user
/// isn't speaking. Fades in/out so the appearance feels like dictation rather
/// than a blocking modal.
struct VoicePartialCaption: View {
    @State private var pipeline = VoicePipeline.shared
    @Environment(\.theme) private var theme

    var body: some View {
        if !self.pipeline.partialTranscript.isEmpty {
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
