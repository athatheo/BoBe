import SwiftUI

/// One-shot, transient caption rendered next to the avatar the first time
/// BoBe starts listening. The voice presence ring + mouth animation are
/// expressive but silent; this caption tells the user, exactly once, what
/// each is for, then quietly disappears.
///
/// Persists "seen" in UserDefaults so it never reappears for a returning
/// user. Owned by the overlay view layer; the store doesn't need to know
/// about it.
struct VoiceCoachmark: View {
    /// True while the caption is on-screen. The parent toggles this on the
    /// first `pipeline.state == .listening` after launch and back off
    /// after the dismissal timer fires (~6s).
    @Binding var isVisible: Bool

    @Environment(\.theme) private var theme

    var body: some View {
        if self.isVisible {
            HStack(spacing: 8) {
                Image(systemName: "waveform.circle.fill")
                    .font(.system(size: 14, weight: .semibold))
                    .foregroundStyle(self.theme.colors.primary)
                VStack(alignment: .leading, spacing: 1) {
                    Text(L10n.tr("overlay.voice.coachmark.title"))
                        .bobeTextStyle(.helper)
                        .fontWeight(.semibold)
                        .foregroundStyle(self.theme.colors.text)
                    Text(L10n.tr("overlay.voice.coachmark.body"))
                        .bobeTextStyle(.overlayStatus)
                        .foregroundStyle(self.theme.colors.textMuted)
                        .lineLimit(2)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .frame(maxWidth: 200, alignment: .leading)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
            .background(
                RoundedRectangle(cornerRadius: 10)
                    .fill(self.theme.colors.background)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 10)
                    .strokeBorder(self.theme.colors.border, lineWidth: 1)
            )
            .shadow(color: self.theme.colors.text.opacity(0.18), radius: 8, y: 3)
            .transition(.opacity.combined(with: .move(edge: .trailing)))
            .accessibilityElement(children: .combine)
        }
    }
}

/// UserDefaults gate for the one-shot coachmark.
enum VoiceCoachmarkFlag {
    private static let key = "bobe.voice.coachmark.seen"

    static var hasSeen: Bool {
        UserDefaults.standard.bool(forKey: key)
    }

    static func markSeen() {
        UserDefaults.standard.set(true, forKey: self.key)
    }
}

#if !SPM_BUILD
    #Preview("Visible") {
        @Previewable @State var visible = true
        return VoiceCoachmark(isVisible: $visible)
            .environment(\.theme, allThemes[0])
            .padding()
            .frame(width: 320, height: 100)
    }
#endif
