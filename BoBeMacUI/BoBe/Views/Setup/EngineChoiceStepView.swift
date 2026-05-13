import SwiftUI

struct EngineChoiceStepView: View {
    @Binding var selection: EngineChoice?
    let onContinue: () -> Void

    @Environment(\.theme) private var theme

    var body: some View {
        VStack(spacing: 18) {
            VStack(spacing: 8) {
                Text(L10n.tr("setup.engine.title"))
                    .bobeTextStyle(.setupTitle)
                    .foregroundStyle(self.theme.colors.text)
                Text(L10n.tr("setup.engine.body"))
                    .bobeTextStyle(.setupBody)
                    .foregroundStyle(self.theme.colors.textMuted)
                    .multilineTextAlignment(.center)
            }
            .padding(.top, 8)

            VStack(spacing: 12) {
                EngineChoiceCard(
                    icon: "cloud.fill",
                    title: L10n.tr("setup.engine.copilot.title"),
                    subtitle: L10n.tr("setup.engine.copilot.subtitle"),
                    isSelected: self.selection == .copilot
                ) {
                    self.selection = .copilot
                }

                EngineChoiceCard(
                    icon: "macbook",
                    title: L10n.tr("setup.engine.local.title"),
                    subtitle: L10n.tr("setup.engine.local.subtitle"),
                    isSelected: self.selection == .local
                ) {
                    self.selection = .local
                }
            }

            Text(L10n.tr("setup.engine.footer"))
                .bobeTextStyle(.helper)
                .foregroundStyle(self.theme.colors.textMuted)
                .multilineTextAlignment(.center)

            Spacer()

            Button(L10n.tr("setup.welcome.continue"), action: self.onContinue)
                .bobeButton(.primary, size: .regular)
                .keyboardShortcut(.defaultAction)
                .disabled(self.selection == nil)
        }
    }
}

private struct EngineChoiceCard: View {
    let icon: String
    let title: String
    let subtitle: String
    let isSelected: Bool
    let onTap: () -> Void

    @Environment(\.theme) private var theme
    @State private var isHovered = false

    var body: some View {
        Button(action: self.onTap) {
            HStack(alignment: .top, spacing: 14) {
                Image(systemName: self.icon)
                    .font(.system(size: 24, weight: .light))
                    .foregroundStyle(self.isSelected ? self.theme.colors.primary : self.theme.colors.textMuted)
                    .frame(width: 36, height: 36)

                VStack(alignment: .leading, spacing: 4) {
                    Text(self.title)
                        .bobeTextStyle(.setupHeading)
                        .foregroundStyle(self.theme.colors.text)
                    Text(self.subtitle)
                        .font(.system(size: 12))
                        .foregroundStyle(self.theme.colors.textMuted)
                        .lineLimit(2)
                        .multilineTextAlignment(.leading)
                }
                .frame(maxWidth: .infinity, alignment: .leading)

                Image(systemName: self.isSelected ? "largecircle.fill.circle" : "circle")
                    .font(.system(size: 18))
                    .foregroundStyle(self.isSelected ? self.theme.colors.primary : self.theme.colors.border)
                    .padding(.top, 6)
            }
            .padding(14)
            .background(
                RoundedRectangle(cornerRadius: 12)
                    .fill(self.isSelected
                        ? self.theme.colors.primary.opacity(0.08)
                        : self.theme.colors.surface)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 12)
                    .stroke(
                        self.isSelected
                            ? self.theme.colors.primary
                            : (self.isHovered ? self.theme.colors.primary.opacity(0.5) : self.theme.colors.border),
                        lineWidth: self.isSelected ? 1.5 : 1
                    )
            )
        }
        .buttonStyle(.plain)
        .onHover { self.isHovered = $0 }
    }
}
