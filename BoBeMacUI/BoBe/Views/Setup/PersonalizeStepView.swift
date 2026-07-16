import SwiftUI

struct PersonalizeStepView: View {
    @Binding var preferredName: String
    @Binding var firstGoal: String
    let onContinue: () -> Void

    @Environment(\.theme) private var theme
    @FocusState private var focusedField: Field?

    private enum Field {
        case name
        case goal
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            VStack(alignment: .leading, spacing: 8) {
                Text(L10n.tr("setup.personalize.title"))
                    .bobeTextStyle(.setupTitle)
                    .foregroundStyle(self.theme.colors.text)
                Text(L10n.tr("setup.personalize.body"))
                    .bobeTextStyle(.setupBody)
                    .foregroundStyle(self.theme.colors.textMuted)
                    .fixedSize(horizontal: false, vertical: true)
            }

            VStack(alignment: .leading, spacing: 8) {
                Text(L10n.tr("setup.personalize.name.label"))
                    .bobeTextStyle(.setupHeading)
                    .foregroundStyle(self.theme.colors.text)
                TextField(
                    L10n.tr("setup.personalize.name.placeholder"),
                    text: self.$preferredName
                )
                .textFieldStyle(.roundedBorder)
                .focused(self.$focusedField, equals: .name)
                .accessibilityIdentifier("setup.personalize.name")
                Text(L10n.tr("setup.personalize.name.help"))
                    .bobeTextStyle(.helper)
                    .foregroundStyle(self.theme.colors.textMuted)
            }

            VStack(alignment: .leading, spacing: 8) {
                Text(L10n.tr("setup.personalize.goal.label"))
                    .bobeTextStyle(.setupHeading)
                    .foregroundStyle(self.theme.colors.text)
                TextField(
                    L10n.tr("setup.personalize.goal.placeholder"),
                    text: self.$firstGoal
                )
                .textFieldStyle(.roundedBorder)
                .focused(self.$focusedField, equals: .goal)
                .accessibilityIdentifier("setup.personalize.goal")
                Text(L10n.tr("setup.personalize.goal.help"))
                    .bobeTextStyle(.helper)
                    .foregroundStyle(self.theme.colors.textMuted)
            }

            Spacer()

            HStack {
                Text(L10n.tr("setup.personalize.optional"))
                    .bobeTextStyle(.helper)
                    .foregroundStyle(self.theme.colors.textMuted)
                Spacer()
                Button(L10n.tr("setup.welcome.continue"), action: self.onContinue)
                    .bobeButton(.primary)
                    .keyboardShortcut(.defaultAction)
                    .accessibilityIdentifier("setup.personalize.continue")
            }
        }
        .onAppear { self.focusedField = .name }
    }
}
