import SwiftUI

/// Surfaces fields needing daemon restart; some subsystems (Checkin, MCP) ignore live PATCH.
struct RestartRequiredBanner: View {
    let fields: Set<String>
    let onDismiss: () -> Void
    @Environment(\.theme) private var theme

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: "arrow.triangle.2.circlepath")
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(self.theme.colors.tertiary)
                .padding(.top, 1)

            VStack(alignment: .leading, spacing: 4) {
                Text(L10n.tr("settings.shared.restart_banner.message"))
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(self.theme.colors.text)

                if !self.fields.isEmpty {
                    Text(self.fields.sorted().joined(separator: ", "))
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(self.theme.colors.textMuted)
                        .lineLimit(2)
                }
            }

            Spacer(minLength: 8)

            Button(L10n.tr("settings.shared.restart_banner.dismiss"), action: self.onDismiss)
                .bobeButton(.ghost, size: .mini)
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 8)
                .fill(self.theme.colors.tertiary.opacity(0.12))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(self.theme.colors.tertiary.opacity(0.4), lineWidth: 1)
        )
    }
}

#if !SPM_BUILD
#Preview("RestartRequiredBanner") {
    RestartRequiredBanner(
        fields: ["checkin_enabled", "checkin_times"],
        onDismiss: {}
    )
    .environment(\.theme, allThemes[0])
    .padding()
    .frame(width: 480)
}
#endif
