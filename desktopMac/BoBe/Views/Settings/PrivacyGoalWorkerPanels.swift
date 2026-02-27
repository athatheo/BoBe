import SwiftUI

/// Privacy settings panel — local-only data posture.
/// Voice/STT/TTS and destructive maintenance flows are intentionally excluded.
struct PrivacyPanel: View {
    @Environment(\.theme) private var theme

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                HStack(spacing: 8) {
                    Image(systemName: "externaldrive.fill")
                        .font(.system(size: 16))
                        .foregroundStyle(theme.colors.primary)
                    Text("Local Storage")
                        .font(.system(size: 16, weight: .semibold))
                        .foregroundStyle(theme.colors.text)
                }

                Text("BoBe stores data locally on this Mac. Souls, goals, memories, user profile, and runtime settings remain on-device.")
                    .font(.system(size: 13))
                    .foregroundStyle(theme.colors.textMuted)

                VStack(alignment: .leading, spacing: 8) {
                    Text("Included in local state")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(theme.colors.text)
                    Text("• Souls and goals\n• Memories and observations\n• User profile and settings\n• Local model metadata")
                        .font(.system(size: 12))
                        .foregroundStyle(theme.colors.textMuted)
                }
                .padding(16)
                .background(
                    RoundedRectangle(cornerRadius: 10)
                        .fill(theme.colors.surface)
                        .stroke(theme.colors.border, lineWidth: 1)
                )
            }
            .padding(24)
        }
    }
}
