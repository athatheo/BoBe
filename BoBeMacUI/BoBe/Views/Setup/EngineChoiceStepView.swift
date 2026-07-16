import SwiftUI

struct EngineChoiceStepView: View {
    @Binding var selection: EngineChoice?
    let onContinue: () -> Void

    @Environment(\.theme) private var theme
    @FocusState private var focused: EngineChoice?

    var body: some View {
        VStack(spacing: 18) {
            VStack(spacing: 10) {
                Text(L10n.tr("setup.engine.hello"))
                    .font(.system(size: 11, weight: .semibold))
                    .tracking(2.2)
                    .textCase(.uppercase)
                    .foregroundStyle(self.theme.colors.textMuted)

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
                    bestFor: L10n.tr("setup.engine.copilot.best_for"),
                    isSelected: self.selection == .copilot
                ) {
                    self.selection = .copilot
                }
                .focused(self.$focused, equals: .copilot)

                EngineChoiceCard(
                    icon: "macbook",
                    title: L10n.tr("setup.engine.local.title"),
                    subtitle: L10n.tr("setup.engine.local.subtitle"),
                    bestFor: L10n.tr("setup.engine.local.best_for"),
                    isSelected: self.selection == .local
                ) {
                    self.selection = .local
                }
                .focused(self.$focused, equals: .local)
            }
            .onKeyPress(.upArrow) {
                self.selection = .copilot
                self.focused = .copilot
                return .handled
            }
            .onKeyPress(.downArrow) {
                self.selection = .local
                self.focused = .local
                return .handled
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
    let bestFor: String
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

                VStack(alignment: .leading, spacing: 6) {
                    Text(self.title)
                        .bobeTextStyle(.setupHeading)
                        .foregroundStyle(self.theme.colors.text)
                    Text(self.subtitle)
                        .bobeTextStyle(.helper)
                        .foregroundStyle(self.theme.colors.textMuted)
                        .lineLimit(2)
                        .multilineTextAlignment(.leading)
                    HStack(spacing: 4) {
                        Image(systemName: "sparkle")
                            .font(.system(size: 9, weight: .semibold))
                            .foregroundStyle(self.theme.colors.primary)
                        Text(self.bestFor)
                            .bobeTextStyle(.helper)
                            .fontWeight(.medium)
                            .foregroundStyle(self.theme.colors.primary)
                    }
                    .padding(.top, 2)
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
