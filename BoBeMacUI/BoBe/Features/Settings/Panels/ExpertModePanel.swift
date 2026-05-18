import SwiftUI

/// Single-toggle settings panel that lives in its own sidebar entry.
/// Gates jargon (codenames, MCP raw JSON, strict offline, Advanced) so
/// novice users see a calm Settings window by default.
struct ExpertModePanel: View {
    @Environment(\.theme) private var theme
    @State private var expert = ExpertMode.shared

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                Text(L10n.tr("settings.expert.title"))
                    .font(.title2.bold())
                    .foregroundStyle(self.theme.colors.text)

                Text(L10n.tr("settings.expert.description"))
                    .font(.system(size: 13))
                    .foregroundStyle(self.theme.colors.textMuted)
                    .fixedSize(horizontal: false, vertical: true)

                CollapsibleSection(
                    title: L10n.tr("settings.expert.title"),
                    icon: "wrench.and.screwdriver.fill",
                    description: nil
                ) {
                    SettingsRow(
                        label: L10n.tr("settings.expert.toggle"),
                        description: L10n.tr("settings.expert.toggle.description")
                    ) {
                        BobeToggle(isOn: Binding(
                            get: { self.expert.isEnabled },
                            set: { self.expert.setEnabled($0) }
                        ))
                    }
                }

                CollapsibleSection(
                    title: L10n.tr("settings.expert.section.what_changes"),
                    icon: "list.bullet.rectangle",
                    description: nil
                ) {
                    Text(L10n.tr("settings.expert.section.what_changes.list"))
                        .font(.system(size: 12))
                        .foregroundStyle(self.theme.colors.textMuted)
                        .fixedSize(horizontal: false, vertical: true)
                        .lineSpacing(4)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
            .padding(24)
        }
    }
}
