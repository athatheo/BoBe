import SwiftUI

enum ProactivityLevel: String, CaseIterable, Sendable {
    case quiet
    case balanced
    case proactive

    struct Settings: Sendable {
        let captureEnabled: Bool
        let captureIntervalSeconds: Int
        let checkinEnabled: Bool
    }

    var settings: Settings {
        switch self {
        case .quiet:
            Settings(captureEnabled: false, captureIntervalSeconds: 120, checkinEnabled: false)
        case .balanced:
            Settings(captureEnabled: true, captureIntervalSeconds: 90, checkinEnabled: false)
        case .proactive:
            Settings(captureEnabled: true, captureIntervalSeconds: 45, checkinEnabled: true)
        }
    }

    var title: String {
        L10n.tr("setup.proactivity.\(self.rawValue).title")
    }

    var description: String {
        L10n.tr("setup.proactivity.\(self.rawValue).description")
    }

    var icon: String {
        switch self {
        case .quiet: "moon.stars"
        case .balanced: "slider.horizontal.3"
        case .proactive: "sparkles"
        }
    }
}

struct ProactivityStepView: View {
    @Binding var selection: ProactivityLevel
    let onContinue: () -> Void

    @Environment(\.theme) private var theme

    var body: some View {
        VStack(spacing: 18) {
            VStack(spacing: 8) {
                Text(L10n.tr("setup.proactivity.title"))
                    .bobeTextStyle(.setupTitle)
                    .foregroundStyle(self.theme.colors.text)
                Text(L10n.tr("setup.proactivity.body"))
                    .bobeTextStyle(.setupBody)
                    .foregroundStyle(self.theme.colors.textMuted)
                    .multilineTextAlignment(.center)
                    .fixedSize(horizontal: false, vertical: true)
            }

            VStack(spacing: 10) {
                ForEach(ProactivityLevel.allCases, id: \.self) { level in
                    Button {
                        self.selection = level
                    } label: {
                        HStack(spacing: 12) {
                            Image(systemName: level.icon)
                                .font(.system(size: 20))
                                .foregroundStyle(
                                    self.selection == level
                                        ? self.theme.colors.primary
                                        : self.theme.colors.textMuted
                                )
                                .frame(width: 28)
                            VStack(alignment: .leading, spacing: 3) {
                                Text(level.title)
                                    .bobeTextStyle(.rowTitle)
                                    .foregroundStyle(self.theme.colors.text)
                                Text(level.description)
                                    .bobeTextStyle(.helper)
                                    .foregroundStyle(self.theme.colors.textMuted)
                                    .multilineTextAlignment(.leading)
                            }
                            Spacer()
                            Image(
                                systemName: self.selection == level
                                    ? "largecircle.fill.circle"
                                    : "circle"
                            )
                            .foregroundStyle(
                                self.selection == level
                                    ? self.theme.colors.primary
                                    : self.theme.colors.border
                            )
                        }
                        .padding(14)
                        .background(
                            RoundedRectangle(cornerRadius: 12)
                                .fill(
                                    self.selection == level
                                        ? self.theme.colors.primary.opacity(0.08)
                                        : self.theme.colors.surface
                                )
                        )
                        .overlay(
                            RoundedRectangle(cornerRadius: 12)
                                .stroke(
                                    self.selection == level
                                        ? self.theme.colors.primary
                                        : self.theme.colors.border,
                                    lineWidth: self.selection == level ? 1.5 : 1
                                )
                        )
                    }
                    .buttonStyle(.plain)
                    .accessibilityIdentifier("setup.proactivity.\(level.rawValue)")
                    .accessibilityAddTraits(self.selection == level ? .isSelected : [])
                }
            }

            Text(L10n.tr("setup.proactivity.footer"))
                .bobeTextStyle(.helper)
                .foregroundStyle(self.theme.colors.textMuted)
                .multilineTextAlignment(.center)

            Spacer()

            Button(L10n.tr("setup.welcome.continue"), action: self.onContinue)
                .bobeButton(.primary)
                .keyboardShortcut(.defaultAction)
                .accessibilityIdentifier("setup.proactivity.continue")
        }
    }
}
