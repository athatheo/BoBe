import SwiftUI

struct WelcomeValueStepView: View {
    let onContinue: () -> Void

    @Environment(\.theme) private var theme

    var body: some View {
        VStack(spacing: 22) {
            Spacer()

            ZStack {
                Circle()
                    .fill(self.theme.colors.primary.opacity(0.12))
                    .frame(width: 104, height: 104)
                Image(systemName: "sparkles")
                    .font(.system(size: 40, weight: .light))
                    .foregroundStyle(self.theme.colors.primary)
            }

            VStack(spacing: 10) {
                Text(L10n.tr("setup.value.title"))
                    .bobeTextStyle(.setupTitle)
                    .foregroundStyle(self.theme.colors.text)
                    .multilineTextAlignment(.center)

                Text(L10n.tr("setup.value.body"))
                    .bobeTextStyle(.setupBody)
                    .foregroundStyle(self.theme.colors.textMuted)
                    .multilineTextAlignment(.center)
                    .lineSpacing(4)
            }

            VStack(alignment: .leading, spacing: 12) {
                self.valueRow(
                    icon: "target",
                    title: L10n.tr("setup.value.goal.title"),
                    body: L10n.tr("setup.value.goal.body")
                )
                self.valueRow(
                    icon: "lock.shield",
                    title: L10n.tr("setup.value.control.title"),
                    body: L10n.tr("setup.value.control.body")
                )
                self.valueRow(
                    icon: "bubble.left.and.bubble.right",
                    title: L10n.tr("setup.value.presence.title"),
                    body: L10n.tr("setup.value.presence.body")
                )
            }
            .padding(16)
            .background(
                RoundedRectangle(cornerRadius: 14)
                    .fill(self.theme.colors.surface)
                    .stroke(self.theme.colors.border, lineWidth: 1)
            )

            Spacer()

            Button(L10n.tr("setup.value.continue"), action: self.onContinue)
                .bobeButton(.primary)
                .keyboardShortcut(.defaultAction)
                .accessibilityIdentifier("setup.value.continue")
        }
    }

    private func valueRow(icon: String, title: String, body: String) -> some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: icon)
                .font(.system(size: 14, weight: .semibold))
                .foregroundStyle(self.theme.colors.primary)
                .frame(width: 20)
                .padding(.top, 2)
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .bobeTextStyle(.rowTitle)
                    .foregroundStyle(self.theme.colors.text)
                Text(body)
                    .bobeTextStyle(.helper)
                    .foregroundStyle(self.theme.colors.textMuted)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}
